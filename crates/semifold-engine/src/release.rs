use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use minijinja::{Environment, UndefinedBehavior, context};
use semifold_core::{
    BumpLevel, ChangesetId, ChangesetInput, DependencyKind, DependencySource, EcosystemId,
    PackageId, PackageReleasePolicy, ReleaseChannel, ReleaseContext, ReleasePlan, ReleasePlanError,
    ReleasePlanner, ReleasePlannerError, ReleasePolicies, WorkspaceGraph,
};
use semifold_resolver::{
    adapter::{AdapterError, EcosystemPlanInput},
    changeset::{BumpLevel as ResolverBumpLevel, Changeset},
    config::{BranchesConfig, ChannelBump, Config, ReleaseChannel as ResolverReleaseChannel},
};
use semver::VersionReq;
use thiserror::Error;

use crate::{
    discovery::{ResolverRegistry, ResolverRegistryError},
    workspace::{WorkspaceLoadError, load_workspace_graph_with_registry},
};

pub struct ReleasePullRequestContext<'a> {
    pub release: &'a ReleaseContext,
    pub branch: String,
    pub changelogs: BTreeMap<PackageId, String>,
    pub oversized_body_notice: &'a str,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RenderedReleasePullRequest {
    pub title: String,
    pub body: String,
}

pub const DEFAULT_RELEASE_COMMIT_MESSAGE: &str = "chore(release): bump versions";
pub const DEFAULT_RELEASE_PULL_REQUEST_TITLE: &str = "chore(release): bump versions";

// A UTF-8 byte budget conservatively satisfies GitHub's character limit too.
const RELEASE_PULL_REQUEST_BODY_LIMIT: usize = 65_536;

pub fn render_release_commit_message(
    template: Option<&str>,
    release: &ReleaseContext,
) -> Result<String, ReleaseMessageRenderError> {
    let message = render_release_message_template(
        template.unwrap_or(DEFAULT_RELEASE_COMMIT_MESSAGE),
        release,
    )?;
    if message.trim().is_empty() {
        return Err(ReleaseMessageRenderError::EmptyCommitMessage);
    }
    Ok(message)
}

pub fn render_release_pull_request_title(
    template: Option<&str>,
    release: &ReleaseContext,
) -> Result<String, ReleaseMessageRenderError> {
    let title = render_release_message_template(
        template.unwrap_or(DEFAULT_RELEASE_PULL_REQUEST_TITLE),
        release,
    )?;
    if title.trim().is_empty() {
        return Err(ReleaseMessageRenderError::EmptyPullRequestTitle);
    }
    if title.contains('\n') || title.contains('\r') {
        return Err(ReleaseMessageRenderError::MultilinePullRequestTitle);
    }
    Ok(title)
}

pub fn render_release_pull_request(
    title: String,
    context: &ReleasePullRequestContext<'_>,
) -> RenderedReleasePullRequest {
    let changelogs = context
        .release
        .plan
        .packages
        .keys()
        .filter_map(|package| {
            context
                .changelogs
                .get(package)
                .map(|changelog| format!("## {}\n\n{changelog}", package.as_str()))
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut body = format!("# Releases\n\n{changelogs}");
    if body.len() > RELEASE_PULL_REQUEST_BODY_LIMIT {
        body = String::from("# Releases\n\n");
        for character in context.oversized_body_notice.chars() {
            if body.len() + character.len_utf8() > RELEASE_PULL_REQUEST_BODY_LIMIT {
                break;
            }
            body.push(character);
        }
        for (package, release) in &context.release.plan.packages {
            // HTML escaping keeps arbitrary configured package IDs on one Markdown line.
            let package = package
                .as_str()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('\r', "&#13;")
                .replace('\n', "&#10;");
            let line = format!(
                "\n\n- <code>{package}</code>: {} → {}",
                release.current_version, release.next_version
            );
            if body.len() + line.len() > RELEASE_PULL_REQUEST_BODY_LIMIT {
                break;
            }
            body.push_str(&line);
        }
    }

    RenderedReleasePullRequest { title, body }
}

fn render_release_message_template(
    template: &str,
    release: &ReleaseContext,
) -> Result<String, ReleaseMessageRenderError> {
    let mut release_value = serde_json::to_value(release)?;
    remove_missing_common_version(&mut release_value, release);

    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    Ok(environment.render_str(template, context!(release => release_value))?)
}

pub fn render_release_branch(
    template: &str,
    release: &ReleaseContext,
) -> Result<String, ReleaseBranchRenderError> {
    let mut release_value = serde_json::to_value(release)?;
    remove_missing_common_version(&mut release_value, release);

    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    let branch = environment.render_str(template, context!(release => release_value))?;
    let reference = format!("refs/heads/{branch}");
    if !git2::Reference::is_valid_name(&reference) {
        return Err(ReleaseBranchRenderError::InvalidGitBranch { branch });
    }
    Ok(branch)
}

pub fn render_configured_release_branch(
    branches: &BranchesConfig,
    release: &ReleaseContext,
) -> Result<String, ReleaseBranchRenderError> {
    let branch = render_release_branch(&branches.release, release)?;
    if branch == branches.base {
        return Err(ReleaseBranchRenderError::MatchesBase { branch });
    }
    Ok(branch)
}

fn remove_missing_common_version(value: &mut serde_json::Value, release: &ReleaseContext) {
    if release.plan.common_version.is_none()
        && let Some(plan) = value
            .as_object_mut()
            .and_then(|release| release.get_mut("plan"))
            .and_then(serde_json::Value::as_object_mut)
    {
        plan.remove("common_version");
    }
}

/// Builds the immutable release plan from the current migration-layer inputs.
pub fn plan_release(
    root: &Path,
    config: &Config,
    changesets: &[Changeset],
) -> Result<ReleasePlan, ReleasePlanningError> {
    let registry = ResolverRegistry::load(root, config)?;
    let graph = load_workspace_graph_with_registry(root, config, &registry)?;
    let changesets = changeset_inputs(changesets);
    let policies = release_policies(&graph, config)?;
    let plan = ReleasePlanner::plan(&graph, &changesets, &policies)?;
    let project_root = camino::Utf8Path::from_path(root).ok_or_else(|| {
        ReleasePlanningError::NonUtf8ProjectRoot {
            path: root.to_path_buf(),
        }
    })?;
    let ecosystems = graph
        .packages()
        .map(|package| package.ecosystem.clone())
        .collect::<BTreeSet<_>>();
    let mut file_edits = Vec::new();
    for ecosystem in ecosystems {
        let workspace_packages = graph
            .packages()
            .filter(|package| package.ecosystem == ecosystem)
            .cloned()
            .collect::<Vec<_>>();
        let released_packages = plan
            .packages()
            .iter()
            .filter(|release| release.ecosystem == ecosystem)
            .map(|release| release.id.clone())
            .collect::<Vec<_>>();
        if released_packages.is_empty() {
            continue;
        }
        let adapter = registry.create_adapter(&ecosystem)?;
        file_edits.extend(adapter.plan_edits(EcosystemPlanInput {
            project_root,
            workspace_packages: &workspace_packages,
            released_packages: &released_packages,
            versions: plan.versions(),
        })?);
    }
    Ok(plan.with_file_edits(file_edits)?)
}

fn changeset_inputs(changesets: &[Changeset]) -> Vec<ChangesetInput> {
    changesets
        .iter()
        .map(|changeset| {
            let mut releases = BTreeMap::<PackageId, BumpLevel>::new();
            for package in &changeset.packages {
                let level = bump_level(package.level);
                releases
                    .entry(PackageId::new(&package.name))
                    .and_modify(|current| *current = (*current).max(level))
                    .or_insert(level);
            }
            ChangesetInput {
                id: ChangesetId::new(&changeset.name),
                releases,
            }
        })
        .collect()
}

fn release_policies(
    graph: &WorkspaceGraph,
    config: &Config,
) -> Result<ReleasePolicies, ReleasePlanningError> {
    graph
        .packages()
        .map(|package| {
            let package_config = config.packages.get(package.id.as_str()).ok_or_else(|| {
                ReleasePlanningError::ConfiguredPackageMissing {
                    package: package.id.clone(),
                }
            })?;
            let mut propagating_dependencies = BTreeMap::new();
            for dependency in &package.dependencies {
                let manifest_runtime = dependency.source == DependencySource::Manifest
                    && package.ecosystem == EcosystemId::RUST
                    && dependency.kind == DependencyKind::Runtime;
                if dependency.source != DependencySource::Config && !manifest_runtime {
                    continue;
                }
                let requirement = if dependency.source == DependencySource::Config {
                    None
                } else {
                    dependency
                        .requirement
                        .as_deref()
                        .map(VersionReq::parse)
                        .transpose()
                        .map_err(
                            |source| ReleasePlanningError::InvalidDependencyRequirement {
                                package: package.id.clone(),
                                dependency: dependency.package.clone(),
                                source,
                            },
                        )?
                };
                if dependency.source == DependencySource::Config {
                    propagating_dependencies.insert(dependency.package.clone(), None);
                } else {
                    propagating_dependencies
                        .entry(dependency.package.clone())
                        .or_insert(requirement);
                }
            }
            Ok((
                package.id.clone(),
                PackageReleasePolicy {
                    channel: release_channel(&package_config.channel),
                    channel_bump: package_config.channel_bump.map(channel_bump),
                    propagating_dependencies,
                },
            ))
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum ReleaseBranchRenderError {
    #[error("failed to serialize release branch context")]
    Serialize(#[from] serde_json::Error),
    #[error("failed to render release branch template")]
    Template(#[from] minijinja::Error),
    #[error("rendered release branch is not a valid Git branch: {branch}")]
    InvalidGitBranch { branch: String },
    #[error("rendered release branch matches the base branch: {branch}")]
    MatchesBase { branch: String },
}

#[derive(Debug, Error)]
pub enum ReleaseMessageRenderError {
    #[error("failed to serialize release message context")]
    Serialize(#[from] serde_json::Error),
    #[error("failed to render release message template")]
    Template(#[from] minijinja::Error),
    #[error("rendered release commit message must not be empty")]
    EmptyCommitMessage,
    #[error("rendered release pull request title must not be empty")]
    EmptyPullRequestTitle,
    #[error("rendered release pull request title must be a single line")]
    MultilinePullRequestTitle,
}

#[derive(Debug, Error)]
pub enum ReleasePlanningError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceLoadError),
    #[error("configured release package is missing: {package}")]
    ConfiguredPackageMissing { package: PackageId },
    #[error("project root is not valid UTF-8: {path:?}")]
    NonUtf8ProjectRoot { path: PathBuf },
    #[error("invalid dependency requirement from {package} to {dependency}")]
    InvalidDependencyRequirement {
        package: PackageId,
        dependency: PackageId,
        #[source]
        source: semver::Error,
    },
    #[error(transparent)]
    Domain(#[from] ReleasePlannerError),
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error(transparent)]
    Registry(#[from] ResolverRegistryError),
    #[error("invalid release plan")]
    ReleasePlan(#[from] ReleasePlanError),
}

const fn bump_level(level: ResolverBumpLevel) -> BumpLevel {
    match level {
        ResolverBumpLevel::Major => BumpLevel::Major,
        ResolverBumpLevel::Minor => BumpLevel::Minor,
        ResolverBumpLevel::Patch => BumpLevel::Patch,
        ResolverBumpLevel::Unchanged => BumpLevel::Unchanged,
    }
}

fn release_channel(channel: &ResolverReleaseChannel) -> ReleaseChannel {
    match channel {
        ResolverReleaseChannel::Stable => ReleaseChannel::Stable,
        ResolverReleaseChannel::Named(name) => ReleaseChannel::Named(name.clone()),
    }
}

const fn channel_bump(bump: ChannelBump) -> BumpLevel {
    match bump {
        ChannelBump::Preserve => BumpLevel::Unchanged,
        ChannelBump::Patch => BumpLevel::Patch,
        ChannelBump::Minor => BumpLevel::Minor,
        ChannelBump::Major => BumpLevel::Major,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use semifold_core::{
        ChangesetId, EditSource, PackageId, PackageRelease, ReleaseContext, ReleasePlan,
        ReleaseReason, VersionMap,
    };
    use semifold_resolver::{
        changeset::Changeset,
        config::{BranchesConfig, PackageConfig},
        resolver::ResolverType,
    };

    use super::*;

    static NEXT_TEMPORARY_ROOT: AtomicU64 = AtomicU64::new(0);

    fn temporary_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "semifold-release-plan-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEMPORARY_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn package(path: &str) -> PackageConfig {
        PackageConfig {
            path: path.into(),
            resolver: ResolverType::Rust.into(),
            publish: None,
            channel: ResolverReleaseChannel::Stable,
            channel_bump: None,
            assets: vec![],
            github_release: None,
            depends_on: vec![],
        }
    }

    fn python_package(path: &str) -> PackageConfig {
        PackageConfig {
            path: path.into(),
            resolver: ResolverType::Python.into(),
            publish: None,
            channel: ResolverReleaseChannel::Stable,
            channel_bump: None,
            assets: vec![],
            github_release: None,
            depends_on: vec![],
        }
    }

    fn node_package(path: &str, depends_on: &[&str]) -> PackageConfig {
        PackageConfig {
            path: path.into(),
            resolver: ResolverType::Nodejs.into(),
            publish: None,
            channel: ResolverReleaseChannel::Stable,
            channel_bump: None,
            assets: vec![],
            github_release: None,
            depends_on: depends_on.iter().copied().map(PackageId::new).collect(),
        }
    }

    fn context(packages: Vec<PackageRelease>) -> ReleaseContext {
        let versions = packages
            .iter()
            .map(|package| (package.id.clone(), package.next_version.clone()))
            .collect::<VersionMap>();
        let order = packages.iter().map(|package| package.id.clone()).collect();
        let plan = ReleasePlan::new(
            packages,
            versions,
            order,
            vec![ChangesetId::new("release-change")],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        ReleaseContext::from_plan(&plan)
    }

    fn planned_package(id: &str, version: semver::Version) -> PackageRelease {
        PackageRelease {
            id: PackageId::new(id),
            ecosystem: EcosystemId::RUST,
            current_version: semver::Version::new(1, 0, 0),
            next_version: version,
            bump: BumpLevel::Minor,
            reasons: vec![ReleaseReason::Changeset {
                changeset: ChangesetId::new("release-change"),
            }],
        }
    }

    #[test]
    fn release_branch_supports_literals_and_release_plan_templates() {
        let context = context(vec![planned_package("core", semver::Version::new(1, 1, 0))]);

        assert_eq!(
            render_release_branch("release", &context).unwrap(),
            "release"
        );
        assert_eq!(
            render_release_branch("release/v{{ release.plan.common_version }}", &context).unwrap(),
            "release/v1.1.0"
        );
        assert_eq!(
            render_release_branch("release/{{ release.plan.fingerprint }}", &context).unwrap(),
            format!("release/{}", context.plan.fingerprint)
        );
    }

    #[test]
    fn release_branch_rejects_missing_common_version_unknown_fields_and_invalid_refs() {
        let context = context(vec![
            planned_package("core", semver::Version::new(1, 1, 0)),
            planned_package("cli", semver::Version::new(2, 0, 0)),
        ]);

        assert!(
            render_release_branch("release/v{{ release.plan.common_version }}", &context).is_err()
        );
        assert!(render_release_branch("{{ release.version }}", &context).is_err());
        assert!(
            render_release_branch(
                "release/{{ release.plan.packages.missing.next_version }}",
                &context
            )
            .is_err()
        );
        assert!(render_release_branch("release branch", &context).is_err());
    }

    #[test]
    fn configured_release_branch_must_not_render_to_the_base_branch() {
        let context = context(vec![planned_package("core", semver::Version::new(1, 1, 0))]);
        let branches = BranchesConfig {
            base: "main".to_string(),
            release: "{{ \"main\" }}".to_string(),
        };

        assert!(matches!(
            render_configured_release_branch(&branches, &context),
            Err(ReleaseBranchRenderError::MatchesBase { branch }) if branch == "main"
        ));
    }

    #[test]
    fn release_messages_preserve_defaults_and_support_release_plan_templates() {
        let context = context(vec![planned_package("core", semver::Version::new(1, 1, 0))]);

        assert_eq!(
            render_release_commit_message(None, &context).unwrap(),
            DEFAULT_RELEASE_COMMIT_MESSAGE
        );
        assert_eq!(
            render_release_pull_request_title(None, &context).unwrap(),
            DEFAULT_RELEASE_PULL_REQUEST_TITLE
        );
        assert_eq!(
            render_release_commit_message(Some("release {{ release.plan.fingerprint }}"), &context)
                .unwrap(),
            format!("release {}", context.plan.fingerprint)
        );
        assert_eq!(
            render_release_pull_request_title(
                Some("Release {{ release.plan.common_version }}"),
                &context
            )
            .unwrap(),
            "Release 1.1.0"
        );
    }

    #[test]
    fn release_messages_reject_invalid_or_unsafe_rendered_values() {
        let context = context(vec![
            planned_package("core", semver::Version::new(1, 1, 0)),
            planned_package("cli", semver::Version::new(2, 0, 0)),
        ]);

        assert!(matches!(
            render_release_commit_message(Some("{{ release.unknown }}"), &context),
            Err(ReleaseMessageRenderError::Template(_))
        ));
        assert!(matches!(
            render_release_commit_message(Some("  "), &context),
            Err(ReleaseMessageRenderError::EmptyCommitMessage)
        ));
        assert!(matches!(
            render_release_pull_request_title(
                Some("Release {{ release.plan.common_version }}"),
                &context
            ),
            Err(ReleaseMessageRenderError::Template(_))
        ));
        assert!(matches!(
            render_release_pull_request_title(Some("Release\nplan"), &context),
            Err(ReleaseMessageRenderError::MultilinePullRequestTitle)
        ));
    }

    #[test]
    fn release_pull_request_uses_the_workspace_context_and_stable_package_order() {
        let release = context(vec![
            planned_package("zeta", semver::Version::new(1, 1, 0)),
            planned_package("alpha", semver::Version::new(1, 1, 0)),
        ]);
        let context = ReleasePullRequestContext {
            release: &release,
            branch: "release/stable".to_string(),
            changelogs: BTreeMap::from([
                (PackageId::new("zeta"), "zeta changes".to_string()),
                (PackageId::new("alpha"), "alpha changes".to_string()),
            ]),
            oversized_body_notice: "See Files changed for full release notes; the summary is size-limited.",
        };

        assert!(std::ptr::eq(context.release, &release));
        assert_eq!(context.branch, "release/stable");
        assert_eq!(
            render_release_pull_request(DEFAULT_RELEASE_PULL_REQUEST_TITLE.to_string(), &context),
            RenderedReleasePullRequest {
                title: "chore(release): bump versions".to_string(),
                body: "# Releases\n\n## alpha\n\nalpha changes\n\n## zeta\n\nzeta changes"
                    .to_string(),
            }
        );
        assert_eq!(
            render_release_pull_request("Custom release".to_string(), &context).title,
            "Custom release"
        );
    }

    #[test]
    fn release_pull_request_preserves_bodies_at_the_byte_limit() {
        let release = context(vec![planned_package("core", semver::Version::new(1, 1, 0))]);
        let prefix = "# Releases\n\n## core\n\n";
        for size in [
            RELEASE_PULL_REQUEST_BODY_LIMIT - 1,
            RELEASE_PULL_REQUEST_BODY_LIMIT,
        ] {
            let changelog = "x".repeat(size - prefix.len());
            let context = ReleasePullRequestContext {
                release: &release,
                branch: "release".to_string(),
                changelogs: BTreeMap::from([(PackageId::new("core"), changelog.clone())]),
                oversized_body_notice: "Full notes are in Files changed.",
            };
            let rendered = render_release_pull_request("Release".to_string(), &context);
            assert_eq!(rendered.body, format!("{prefix}{changelog}"));
            assert_eq!(rendered.body.len(), size);
        }
    }

    #[test]
    fn release_pull_request_summarizes_oversized_ascii_and_unicode_notes() {
        let release = context(vec![
            planned_package("zeta", semver::Version::new(2, 0, 0)),
            planned_package("alpha", semver::Version::new(1, 1, 0)),
        ]);
        for changelog in [
            "x".repeat(RELEASE_PULL_REQUEST_BODY_LIMIT - "# Releases\n\n## alpha\n\n".len() + 1),
            "修复🚀".repeat(10_000),
        ] {
            let context = ReleasePullRequestContext {
                release: &release,
                branch: "release/alpha".to_string(),
                changelogs: BTreeMap::from([(PackageId::new("alpha"), changelog.clone())]),
                oversized_body_notice: "完整变更日志见 Files changed；摘要可能省略部分包。",
            };
            let rendered = render_release_pull_request("Custom release".to_string(), &context);
            assert_eq!(rendered.title, "Custom release");
            assert_eq!(
                rendered.body,
                concat!(
                    "# Releases\n\n完整变更日志见 Files changed；摘要可能省略部分包。",
                    "\n\n- <code>alpha</code>: 1.0.0 → 1.1.0",
                    "\n\n- <code>zeta</code>: 1.0.0 → 2.0.0",
                )
            );
            assert!(rendered.body.len() <= RELEASE_PULL_REQUEST_BODY_LIMIT);
            assert_eq!(context.changelogs[&PackageId::new("alpha")], changelog);
        }
    }

    #[test]
    fn release_pull_request_bounds_large_package_summaries_on_complete_lines() {
        let release = context(
            (0..2_000)
                .map(|index| {
                    planned_package(
                        &format!("package-{index:04}"),
                        semver::Version::new(1, 1, 0),
                    )
                })
                .collect(),
        );
        let context = ReleasePullRequestContext {
            release: &release,
            branch: "release".to_string(),
            changelogs: BTreeMap::from([(
                PackageId::new("package-0000"),
                "x".repeat(RELEASE_PULL_REQUEST_BODY_LIMIT),
            )]),
            oversized_body_notice: "See Files changed for all packages; some summaries may be omitted.",
        };
        let rendered = render_release_pull_request("Release".to_string(), &context);
        assert!(rendered.body.len() <= RELEASE_PULL_REQUEST_BODY_LIMIT);
        assert!(rendered.body.contains("<code>package-0000</code>"));
        assert!(!rendered.body.contains("<code>package-1999</code>"));
        assert!(rendered.body.ends_with("1.0.0 → 1.1.0"));
        assert!(
            rendered.body.len() + "\n\n- <code>package-1999</code>: 1.0.0 → 1.1.0".len()
                > RELEASE_PULL_REQUEST_BODY_LIMIT
        );
    }

    #[test]
    fn release_pull_request_bounds_long_notices_and_escapes_summary_names() {
        let package = "包<&>\nname";
        let release = context(vec![planned_package(
            package,
            semver::Version::new(1, 1, 0),
        )]);
        let long_notice = "说明🚀".repeat(RELEASE_PULL_REQUEST_BODY_LIMIT);
        let mut context = ReleasePullRequestContext {
            release: &release,
            branch: "release".to_string(),
            changelogs: BTreeMap::from([(
                PackageId::new(package),
                "x".repeat(RELEASE_PULL_REQUEST_BODY_LIMIT),
            )]),
            oversized_body_notice: &long_notice,
        };
        let rendered = render_release_pull_request("Release".to_string(), &context);
        assert!(rendered.body.len() <= RELEASE_PULL_REQUEST_BODY_LIMIT);
        assert!(RELEASE_PULL_REQUEST_BODY_LIMIT - rendered.body.len() < 4);
        assert!(!rendered.body.contains("<code>"));

        context.oversized_body_notice = "See Files changed.";
        let rendered = render_release_pull_request("Release".to_string(), &context);
        assert!(
            rendered
                .body
                .contains("<code>包&lt;&amp;&gt;&#10;name</code>")
        );
    }

    #[test]
    fn bridges_resolver_inputs_into_the_core_release_plan() {
        let root = temporary_root();
        for (path, manifest) in [
            ("core", "[package]\nname = \"core\"\nversion = \"1.0.0\"\n"),
            (
                "app",
                "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[dependencies]\ncore = { version = \"^1.0.0\", path = \"../core\" }\n",
            ),
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
            fs::write(root.join(path).join("Cargo.toml"), manifest).unwrap();
        }
        let config = Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages: BTreeMap::from([
                ("app".to_string(), package("app")),
                ("core".to_string(), package("core")),
            ]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        };
        let mut changeset = Changeset::new("core-major".to_string(), &root);
        changeset.add_package("core".to_string(), ResolverBumpLevel::Major, None);

        let plan = plan_release(&root, &config, std::slice::from_ref(&changeset)).unwrap();
        let repeated = plan_release(&root, &config, std::slice::from_ref(&changeset)).unwrap();

        assert_eq!(repeated, plan);

        assert_eq!(
            plan.order(),
            [PackageId::new("core"), PackageId::new("app")]
        );
        let app = plan.package(&PackageId::new("app")).unwrap();
        assert_eq!(app.bump, BumpLevel::Patch);
        assert!(matches!(
            app.reasons.as_slice(),
            [ReleaseReason::DependencyPropagation { dependency, .. }]
                if dependency == &PackageId::new("core")
        ));
        assert_eq!(
            plan.file_edits()
                .iter()
                .map(|edit| edit.path.as_str())
                .collect::<Vec<_>>(),
            ["app/Cargo.toml", "core/Cargo.toml"]
        );
        assert!(
            plan.file_edits()[0]
                .new_content
                .contains("version = \"2.0.0\"")
        );
        assert!(
            plan.file_edits()[1]
                .new_content
                .contains("version = \"2.0.0\"")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plans_workspace_inherited_versions_as_one_release_closure_and_edit() {
        let root = temporary_root();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        for (name, publish) in [("public", ""), ("private", "publish = false\n")] {
            let package_root = root.join("crates").join(name);
            fs::create_dir_all(&package_root).unwrap();
            fs::write(
                package_root.join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\nversion.workspace = true\n{publish}"),
            )
            .unwrap();
        }
        let config = Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages: BTreeMap::from([
                ("private".to_string(), package("crates/private")),
                ("public".to_string(), package("crates/public")),
            ]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        };
        let mut changeset = Changeset::new("private-feature".to_string(), &root);
        changeset.add_package("private".to_string(), ResolverBumpLevel::Minor, None);

        let plan = plan_release(&root, &config, &[changeset]).unwrap();

        assert_eq!(
            plan.order(),
            [PackageId::new("private"), PackageId::new("public")]
        );
        assert!(
            plan.packages()
                .iter()
                .all(|package| { package.next_version == semver::Version::new(1, 3, 0) })
        );
        assert_eq!(plan.file_edits().len(), 1);
        assert_eq!(plan.file_edits()[0].path, "Cargo.toml");
        assert!(matches!(
            &plan.file_edits()[0].source,
            EditSource::WorkspaceManifest { shared_versions, .. }
                if shared_versions[0].packages
                    == [PackageId::new("private"), PackageId::new("public")]
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_cross_ecosystem_dependency_propagates_a_patch_release() {
        let root = temporary_root();
        fs::create_dir_all(root.join("rust-core")).unwrap();
        fs::create_dir_all(root.join("node-binding")).unwrap();
        fs::write(
            root.join("rust-core/Cargo.toml"),
            "[package]\nname = \"native-core\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        fs::write(
            root.join("node-binding/package.json"),
            r#"{"name":"node-binding","version":"1.0.0"}"#,
        )
        .unwrap();
        let config = Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages: BTreeMap::from([
                (
                    "node-binding".to_string(),
                    node_package("node-binding", &["rust-core"]),
                ),
                ("rust-core".to_string(), package("rust-core")),
            ]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        };
        let mut changeset = Changeset::new("native-core-minor".to_string(), &root);
        changeset.add_package("rust-core".to_string(), ResolverBumpLevel::Minor, None);

        let plan = plan_release(&root, &config, &[changeset]).unwrap();

        assert_eq!(
            plan.order(),
            [PackageId::new("rust-core"), PackageId::new("node-binding")]
        );
        let binding = plan.package(&PackageId::new("node-binding")).unwrap();
        assert_eq!(binding.bump, BumpLevel::Patch);
        assert_eq!(binding.next_version, semver::Version::new(1, 0, 1));
        assert!(matches!(
            binding.reasons.as_slice(),
            [ReleaseReason::DependencyPropagation { dependency, .. }]
                if dependency == &PackageId::new("rust-core")
        ));
        let node_edit = plan
            .file_edits()
            .iter()
            .find(|edit| edit.path == "node-binding/package.json")
            .unwrap();
        assert!(node_edit.new_content.contains(r#""version": "1.0.1""#));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rust_development_and_build_dependencies_order_without_propagating() {
        let root = temporary_root();
        for (path, manifest) in [
            ("core", "[package]\nname = \"core\"\nversion = \"1.0.0\"\n"),
            (
                "app",
                "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[dev-dependencies]\ncore = { version = \"^1.0.0\", path = \"../core\" }\n\n[build-dependencies]\ncore = { version = \"^1.0.0\", path = \"../core\" }\n",
            ),
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
            fs::write(root.join(path).join("Cargo.toml"), manifest).unwrap();
        }
        let config = Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages: BTreeMap::from([
                ("app".to_string(), package("app")),
                ("core".to_string(), package("core")),
            ]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        };
        let mut changeset = Changeset::new("core-major".to_string(), &root);
        changeset.add_package("core".to_string(), ResolverBumpLevel::Major, None);

        let plan = plan_release(&root, &config, &[changeset]).unwrap();

        assert_eq!(plan.order(), [PackageId::new("core")]);
        assert!(plan.package(&PackageId::new("app")).is_none());
        assert!(
            plan.file_edits()
                .iter()
                .all(|edit| edit.path != "app/Cargo.toml")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn node_manifest_dependency_kinds_do_not_propagate_without_depends_on() {
        let root = temporary_root();
        fs::create_dir_all(root.join("core")).unwrap();
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("core/package.json"),
            r#"{"name":"core","version":"1.0.0"}"#,
        )
        .unwrap();
        fs::write(
            root.join("app/package.json"),
            r#"{
  "name": "app",
  "version": "1.0.0",
  "dependencies": { "core": "^1.0.0" },
  "devDependencies": { "core": "^1.0.0" },
  "peerDependencies": { "core": "^1.0.0" },
  "optionalDependencies": { "core": "^1.0.0" }
}"#,
        )
        .unwrap();
        let config = Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages: BTreeMap::from([
                ("app".to_string(), node_package("app", &[])),
                ("core".to_string(), node_package("core", &[])),
            ]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        };
        let mut changeset = Changeset::new("core-major".to_string(), &root);
        changeset.add_package("core".to_string(), ResolverBumpLevel::Major, None);

        let plan = plan_release(&root, &config, &[changeset]).unwrap();

        assert_eq!(plan.order(), [PackageId::new("core")]);
        assert!(plan.package(&PackageId::new("app")).is_none());
        assert!(
            plan.file_edits()
                .iter()
                .all(|edit| edit.path != "app/package.json")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plans_one_shared_workspace_dependency_edit_for_aliased_package_ids() {
        let root = temporary_root();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"app\", \"core\"]\n\n[workspace.dependencies]\ncore-alias = { package = \"core\", version = \"^1.0.0\", path = \"core\" }\n",
        )
        .unwrap();
        for (path, manifest) in [
            ("core", "[package]\nname = \"core\"\nversion = \"1.0.0\"\n"),
            (
                "app",
                "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[dependencies]\ncore-alias = { workspace = true }\n",
            ),
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
            fs::write(root.join(path).join("Cargo.toml"), manifest).unwrap();
        }
        let config = Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages: BTreeMap::from([
                ("app-id".to_string(), package("app")),
                ("core-id".to_string(), package("core")),
            ]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        };
        let mut changeset = Changeset::new("core-major".to_string(), &root);
        changeset.add_package("core-id".to_string(), ResolverBumpLevel::Major, None);

        let plan = plan_release(&root, &config, &[changeset]).unwrap();

        assert_eq!(
            plan.order(),
            [PackageId::new("core-id"), PackageId::new("app-id")]
        );
        assert_eq!(
            plan.file_edits()
                .iter()
                .map(|edit| edit.path.as_str())
                .collect::<Vec<_>>(),
            ["Cargo.toml", "app/Cargo.toml", "core/Cargo.toml"]
        );
        let workspace_edit = plan
            .file_edits()
            .iter()
            .find(|edit| edit.path == "Cargo.toml")
            .unwrap();
        assert!(
            workspace_edit.new_content.contains(
                "core-alias = { package = \"core\", version = \"2.0.0\", path = \"core\" }"
            )
        );
        assert_eq!(
            plan.file_edits()
                .iter()
                .filter(|edit| edit.path == "Cargo.toml")
                .count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dynamic_python_version_can_plan_a_release_without_writing_cargo() {
        let root = temporary_root();
        fs::write(
            root.join("pyproject.toml"),
            "[project]\nname = \"native-example\"\ndynamic = [\"version\"]\n",
        )
        .unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"native-example\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        let config = Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages: BTreeMap::from([("native-example".to_string(), python_package("."))]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        };
        let mut changeset = Changeset::new("python-patch".to_string(), &root);
        changeset.add_package("native-example".to_string(), ResolverBumpLevel::Patch, None);

        let plan = plan_release(&root, &config, &[changeset]).unwrap();

        assert_eq!(
            plan.package(&PackageId::new("native-example"))
                .unwrap()
                .next_version,
            semver::Version::new(1, 0, 1)
        );
        assert!(plan.file_edits().is_empty());
        assert!(
            fs::read_to_string(root.join("Cargo.toml"))
                .unwrap()
                .contains("version = \"1.0.0\"")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plans_python_post_release_with_pep_440_manifest_version() {
        let root = temporary_root();
        fs::write(
            root.join("pyproject.toml"),
            "[project]\nname = \"example\"\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        let mut package = python_package(".");
        package.channel = ResolverReleaseChannel::Named("post".to_string());
        let config = Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages: BTreeMap::from([("example".to_string(), package)]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        };
        let mut changeset = Changeset::new("python-post".to_string(), &root);
        changeset.add_package("example".to_string(), ResolverBumpLevel::Patch, None);

        let plan = plan_release(&root, &config, &[changeset]).unwrap();

        assert_eq!(
            plan.package(&PackageId::new("example"))
                .unwrap()
                .next_version,
            semver::Version::parse("1.2.4-post.0").unwrap()
        );
        assert!(
            plan.file_edits()
                .iter()
                .any(|edit| edit.new_content.contains("version = \"1.2.4.post0\""))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
