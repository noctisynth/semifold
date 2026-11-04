use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use semifold_changelog::{
    ChangelogRenderer, ChangelogSource, GenerateChangelogError, generate_changelog,
    utils::render_changelog,
};
use semifold_core::{
    ChangesetId, DependencyUpdateContext, EditSource, FileEdit, FileEditExpectation, FileHash,
    PackageId, ReleaseContext, ReleasePackageContext, ReleasePackageContextError, ReleasePlan,
    ReleasePlanError, ReleaseReason, RepositoryContext,
};
use semifold_resolver::{
    changeset::Changeset,
    config::{PackageConfig, StdioType},
    error::ResolveError,
};
use thiserror::Error;

use crate::{
    file_edit_executor::{
        FileEditApplyError, FileEditApplyReport, FileEditExecutor, validate_file_edits,
    },
    project::Project,
    publish_plan::{CommandPhase, CommandSpec, StdioPolicy},
    publisher::{CommandError, CommandRunner},
    release::render_configured_release_branch,
    workspace::{WorkspaceLoadError, load_workspace_graph},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReleaseExecutionOptions {
    pub collect_remote_metadata: bool,
    pub repository: Option<RepositoryContext>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionMode {
    Apply,
    DryRun,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostVersionCommand {
    pub package: PackageId,
    pub command: CommandSpec,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostVersionCommandOutcome {
    Success,
    Failed,
}

pub enum PostVersionCommandEvent {
    Started,
    Finished(PostVersionCommandOutcome),
}

#[derive(Debug)]
pub struct ReleaseApplyPlan {
    pub release: ReleasePlan,
    pub release_context: ReleaseContext,
    pub release_branch: String,
    pub project_root: Utf8PathBuf,
    pub config_path: Utf8PathBuf,
    pub changelogs: BTreeMap<PackageId, String>,
    pub changesets_to_remove: Vec<Utf8PathBuf>,
    pub channel_bumps_to_consume: Vec<PackageId>,
    pub post_version_commands: Vec<PostVersionCommand>,
    pub remote_metadata_failures: Vec<PackageId>,
    pub github_failures: Vec<(PackageId, semifold_changelog::github::GitHubFailure)>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApplyReport {
    pub changelogs: BTreeMap<PackageId, String>,
    pub file_edits: Option<FileEditApplyReport>,
    pub unconsumed_changesets: Vec<ChangesetId>,
}

#[derive(Debug)]
pub struct PostVersionFailure {
    pub package: PackageId,
    pub command: CommandSpec,
    pub source: CommandError,
}

#[derive(Debug, Error)]
pub enum ReleaseApplyError {
    #[error("release file validation failed")]
    FileValidation(#[source] FileEditApplyError),
    #[error("release file application failed")]
    FileApply(#[source] FileEditApplyError),
    #[error("post-version command failed for {failure_package}", failure_package = .failure.package)]
    PostVersion {
        report: Box<ApplyReport>,
        failure: Box<PostVersionFailure>,
    },
    #[error("failed to update one-shot channel bumps")]
    ChannelBump(#[source] ChannelBumpError),
    #[error("failed to remove consumed changeset {path}")]
    RemoveChangeset {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub async fn prepare_release(
    project: &Project,
    changesets: &[Changeset],
    release: ReleasePlan,
    options: &ReleaseExecutionOptions,
) -> Result<ReleaseApplyPlan, ReleasePrepareError> {
    let repository = crate::git::open_repository(project.root.as_std_path()).map_err(|source| {
        ReleasePrepareError::GitOpen {
            path: project.root.clone(),
            source,
        }
    })?;
    let workspace = load_workspace_graph(project.root.as_std_path(), &project.config)?;
    let mut release_context = ReleaseContext::from_plan(&release);
    if let Some(repository) = options.repository.clone() {
        release_context = release_context.with_repository(repository);
    }
    let release_branch =
        render_configured_release_branch(&project.config.branches, &release_context)
            .map_err(ReleasePrepareError::ReleaseBranch)?;
    let changelog_renderer = release
        .order()
        .first()
        .map(|package| ChangelogRenderer::new(&project.config.changelog, package))
        .transpose()
        .map_err(GenerateChangelogError::from)?;
    let mut file_edits = release.file_edits().to_vec();
    let mut changelogs = BTreeMap::new();
    let mut remote_metadata_failures = Vec::new();
    let mut github_failures = Vec::new();

    for package_id in release.order() {
        let renderer = changelog_renderer.as_ref().ok_or_else(|| {
            ReleasePrepareError::ChangelogRendererMissing {
                package: package_id.clone(),
            }
        })?;
        let package_name = package_id.as_str();
        let package_config = project.config.packages.get(package_name).ok_or_else(|| {
            ReleasePrepareError::ConfiguredPackageMissing {
                package: package_id.clone(),
            }
        })?;
        let package_release = release.package(package_id).ok_or_else(|| {
            ReleasePrepareError::PlannedPackageMissing {
                package: package_id.clone(),
            }
        })?;
        let dependency_updates = package_release
            .reasons
            .iter()
            .filter_map(|reason| match reason {
                ReleaseReason::DependencyPropagation { dependency, .. } => release
                    .versions()
                    .get(dependency)
                    .map(|version| DependencyUpdateContext {
                        package: dependency.clone(),
                        next_version: version.clone(),
                    }),
                ReleaseReason::Changeset { .. }
                | ReleaseReason::SharedVersionPropagation { .. } => None,
            })
            .collect::<Vec<_>>();
        let snapshot = workspace.package(package_id).ok_or_else(|| {
            ReleasePrepareError::WorkspacePackageMissing {
                package: package_id.clone(),
            }
        })?;
        let package_context = ReleasePackageContext::from_snapshot(&release_context, snapshot)?;
        let changelog = generate_changelog(
            renderer,
            &ChangelogSource {
                repo_root: project.root.as_std_path(),
                tags: &project.config.tags,
                repository: options.repository.as_ref(),
            },
            &repository,
            changesets,
            package_context,
            dependency_updates,
            options.collect_remote_metadata,
        )
        .await?;
        github_failures.extend(
            changelog
                .github_failures
                .into_iter()
                .map(|error| (package_id.clone(), error)),
        );
        if changelog.remote_metadata_failed {
            remote_metadata_failures.push(package_id.clone());
        }
        file_edits.push(plan_changelog_edit(
            &project.root,
            package_config,
            package_id,
            &changelog.content,
            &package_release.next_version.to_string(),
            changelog.requires_marker,
        )?);
        changelogs.insert(package_id.clone(), changelog.content);
    }

    let release = release.with_file_edits(file_edits)?;
    validate_file_edits(&project.root, release.file_edits())
        .map_err(ReleasePrepareError::FileValidation)?;
    let changesets_to_remove = consumed_changeset_paths(changesets, release.consumed_changesets())?;
    let channel_bumps_to_consume = release
        .packages()
        .iter()
        .filter(|package| {
            package.current_version.pre.is_empty()
                && project
                    .config
                    .packages
                    .get(package.id.as_str())
                    .is_some_and(|config| {
                        config.channel_bump.is_some() && !config.channel.is_stable()
                    })
        })
        .map(|package| package.id.clone())
        .collect();

    Ok(ReleaseApplyPlan {
        release,
        release_context,
        release_branch,
        project_root: project.root.clone(),
        config_path: project.config_path.clone(),
        changelogs,
        changesets_to_remove,
        channel_bumps_to_consume,
        post_version_commands: plan_post_version_commands(project)?,
        remote_metadata_failures,
        github_failures,
    })
}

pub fn apply_release<D>(
    deps: &D,
    plan: ReleaseApplyPlan,
    mode: ExecutionMode,
) -> Result<ApplyReport, ReleaseApplyError>
where
    D: crate::service::EngineDependencies + CommandRunner,
{
    apply_release_with_callback(deps, plan, mode, &mut ignore_post_version_event)
}

pub fn apply_release_with_callback<D, F>(
    deps: &D,
    plan: ReleaseApplyPlan,
    mode: ExecutionMode,
    callback: &mut F,
) -> Result<ApplyReport, ReleaseApplyError>
where
    D: crate::service::EngineDependencies + CommandRunner,
    F: FnMut(&PostVersionCommand, PostVersionCommandEvent),
{
    validate_file_edits(&plan.project_root, plan.release.file_edits())
        .map_err(ReleaseApplyError::FileValidation)?;
    let changeset_ids = plan.release.consumed_changesets().to_vec();

    if mode == ExecutionMode::DryRun {
        run_post_version_commands(
            deps,
            &plan.post_version_commands,
            mode,
            None,
            &plan,
            callback,
        )?;
        return Ok(ApplyReport {
            changelogs: plan.changelogs,
            file_edits: None,
            unconsumed_changesets: changeset_ids,
        });
    }

    let file_edits = FileEditExecutor::new(&plan.project_root)
        .apply(plan.release.file_edits())
        .map_err(ReleaseApplyError::FileApply)?;
    run_post_version_commands(
        deps,
        &plan.post_version_commands,
        mode,
        Some(file_edits.clone()),
        &plan,
        callback,
    )?;
    consume_channel_bumps(deps, &plan.config_path, &plan.channel_bumps_to_consume)
        .map_err(ReleaseApplyError::ChannelBump)?;
    for path in &plan.changesets_to_remove {
        deps.remove_file(path)
            .map_err(|source| ReleaseApplyError::RemoveChangeset {
                path: path.clone(),
                source,
            })?;
    }

    Ok(ApplyReport {
        changelogs: plan.changelogs,
        file_edits: Some(file_edits),
        unconsumed_changesets: Vec::new(),
    })
}

fn run_post_version_commands<D, F>(
    deps: &D,
    commands: &[PostVersionCommand],
    mode: ExecutionMode,
    file_edits: Option<FileEditApplyReport>,
    plan: &ReleaseApplyPlan,
    callback: &mut F,
) -> Result<(), ReleaseApplyError>
where
    D: CommandRunner,
    F: FnMut(&PostVersionCommand, PostVersionCommandEvent),
{
    for planned in commands {
        if mode == ExecutionMode::DryRun && !planned.command.run_in_dry_run {
            continue;
        }
        callback(planned, PostVersionCommandEvent::Started);
        if let Err(source) = deps.run(&planned.command) {
            callback(
                planned,
                PostVersionCommandEvent::Finished(PostVersionCommandOutcome::Failed),
            );
            return Err(ReleaseApplyError::PostVersion {
                report: Box::new(ApplyReport {
                    changelogs: plan.changelogs.clone(),
                    file_edits,
                    unconsumed_changesets: plan.release.consumed_changesets().to_vec(),
                }),
                failure: Box::new(PostVersionFailure {
                    package: planned.package.clone(),
                    command: planned.command.clone(),
                    source,
                }),
            });
        }
        callback(
            planned,
            PostVersionCommandEvent::Finished(PostVersionCommandOutcome::Success),
        );
    }
    Ok(())
}

fn ignore_post_version_event(_command: &PostVersionCommand, _event: PostVersionCommandEvent) {}

fn consumed_changeset_paths(
    changesets: &[Changeset],
    consumed: &[ChangesetId],
) -> Result<Vec<Utf8PathBuf>, ReleasePrepareError> {
    consumed
        .iter()
        .map(|id| {
            let changeset = changesets
                .iter()
                .find(|changeset| changeset.name == id.as_str())
                .ok_or_else(|| ReleasePrepareError::ChangesetMissing { id: id.clone() })?;
            let path = changeset
                .path
                .as_ref()
                .ok_or_else(|| ReleasePrepareError::ChangesetSourceMissing { id: id.clone() })?;
            Utf8PathBuf::from_path_buf(path.clone())
                .map_err(|path| ReleasePrepareError::NonUtf8Path { path })
        })
        .collect()
}

fn plan_post_version_commands(
    project: &Project,
) -> Result<Vec<PostVersionCommand>, ReleasePrepareError> {
    let mut commands = Vec::new();
    for (package_name, package_config) in &project.config.packages {
        let Some(resolver_config) = project.config.resolver.get(&package_config.resolver) else {
            continue;
        };
        let working_directory =
            project
                .root
                .join(Utf8Path::from_path(&package_config.path).ok_or_else(|| {
                    ReleasePrepareError::NonUtf8Path {
                        path: package_config.path.clone(),
                    }
                })?);
        for command in &resolver_config.post_version {
            commands.push(PostVersionCommand {
                package: PackageId::new(package_name),
                command: CommandSpec {
                    executable: command.command.clone(),
                    args: command.args.clone().unwrap_or_default(),
                    environment: command.extra_env.clone(),
                    working_directory: working_directory.clone(),
                    phase: CommandPhase::PostVersion,
                    stdout: stdio_policy(command.stdout),
                    stderr: stdio_policy(command.stderr),
                    run_in_dry_run: command.dry_run.unwrap_or(false),
                },
            });
        }
    }
    Ok(commands)
}

fn plan_changelog_edit(
    root: &Utf8Path,
    package_config: &PackageConfig,
    package: &PackageId,
    entry: &str,
    version: &str,
    require_marker: bool,
) -> Result<FileEdit, ReleasePrepareError> {
    let relative_path = Utf8PathBuf::from_path_buf(package_config.path.join("CHANGELOG.md"))
        .map_err(|path| ReleasePrepareError::NonUtf8Path { path })?;
    let absolute_path = root.join(&relative_path);
    let content = match std::fs::read_to_string(&absolute_path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(ReleasePrepareError::ReadChangelog {
                path: absolute_path,
                source,
            });
        }
    };
    let expected = content
        .as_ref()
        .map_or(FileEditExpectation::Missing, |content| {
            FileEditExpectation::Existing {
                hash: FileHash::from_bytes(content.as_bytes()),
            }
        });
    let new_content = render_changelog(
        &absolute_path,
        content.as_deref(),
        entry,
        version,
        require_marker,
    )
    .map_err(GenerateChangelogError::from)?;

    Ok(FileEdit {
        path: relative_path,
        expected,
        new_content,
        source: EditSource::Changelog {
            package: package.clone(),
        },
    })
}

fn consume_channel_bumps<D>(
    deps: &D,
    path: &Utf8Path,
    packages: &[PackageId],
) -> Result<(), ChannelBumpError>
where
    D: crate::service::EngineDependencies,
{
    if packages.is_empty() {
        return Ok(());
    }
    let original = std::fs::read_to_string(path).map_err(|source| ChannelBumpError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut document = original
        .parse::<toml_edit::DocumentMut>()
        .map_err(|source| ChannelBumpError::Parse {
            path: path.to_owned(),
            source,
        })?;
    let configured = document
        .get_mut("packages")
        .and_then(toml_edit::Item::as_table_mut)
        .ok_or(ChannelBumpError::MissingPackagesTable)?;
    for package in packages {
        let table = configured
            .get_mut(package.as_str())
            .and_then(toml_edit::Item::as_table_like_mut)
            .ok_or_else(|| ChannelBumpError::PackageNotTable {
                package: package.clone(),
            })?;
        table.remove("channel-bump");
    }
    let content = document.to_string();
    if content != original {
        semifold_resolver::config::load_config_from_str(path.as_std_path(), &content)
            .map_err(ChannelBumpError::InvalidConfig)?;
        deps.write_atomic(path, &content)
            .map_err(|source| ChannelBumpError::Write {
                path: path.to_owned(),
                source,
            })?;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum ReleasePrepareError {
    #[error("failed to open Git repository at {path}: {source:#}")]
    GitOpen {
        path: Utf8PathBuf,
        #[source]
        source: gix::Error,
    },
    #[error(transparent)]
    Workspace(#[from] WorkspaceLoadError),
    #[error("configured release package is missing: {package}")]
    ConfiguredPackageMissing { package: PackageId },
    #[error("planned release package is missing: {package}")]
    PlannedPackageMissing { package: PackageId },
    #[error("workspace release package is missing: {package}")]
    WorkspacePackageMissing { package: PackageId },
    #[error(transparent)]
    ReleaseContext(#[from] ReleasePackageContextError),
    #[error("failed to render release branch")]
    ReleaseBranch(#[source] crate::release::ReleaseBranchRenderError),
    #[error("failed to generate changelog")]
    Changelog(#[from] GenerateChangelogError),
    #[error("changelog renderer is unavailable for planned package {package}")]
    ChangelogRendererMissing { package: PackageId },
    #[error("failed to read changelog {path}")]
    ReadChangelog {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("release path is not valid UTF-8: {path:?}")]
    NonUtf8Path { path: std::path::PathBuf },
    #[error("invalid prepared release plan")]
    ReleasePlan(#[from] ReleasePlanError),
    #[error("release file validation failed")]
    FileValidation(#[source] FileEditApplyError),
    #[error("planned changeset is missing: {id}")]
    ChangesetMissing { id: ChangesetId },
    #[error("planned changeset has no source path: {id}")]
    ChangesetSourceMissing { id: ChangesetId },
}

#[derive(Debug, Error)]
pub enum ChannelBumpError {
    #[error("failed to read configuration {path}")]
    Read {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse configuration {path}")]
    Parse {
        path: Utf8PathBuf,
        #[source]
        source: toml_edit::TomlError,
    },
    #[error("configuration is missing the packages table")]
    MissingPackagesTable,
    #[error("configured package {package} is not a table")]
    PackageNotTable { package: PackageId },
    #[error("configuration is invalid after consuming channel bumps")]
    InvalidConfig(#[source] ResolveError),
    #[error("failed to write configuration {path}")]
    Write {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
}

const fn stdio_policy(stdio: StdioType) -> StdioPolicy {
    match stdio {
        StdioType::Inherit => StdioPolicy::Inherit,
        StdioType::Pipe => StdioPolicy::Pipe,
        StdioType::Null => StdioPolicy::Null,
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

    use semifold_core::{FileEditExpectation, PackageId};
    use semifold_resolver::{
        config::{PackageConfig, ReleaseChannel},
        resolver::ResolverType,
    };

    use super::plan_changelog_edit;

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

    fn temporary_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock in tests must be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "semifold-release-apply-{}-{nonce}-{}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("release apply fixture directory must be created");
        root
    }

    fn package_config() -> PackageConfig {
        PackageConfig {
            path: "package".into(),
            resolver: ResolverType::Nodejs.into(),
            publish: None,
            channel: ReleaseChannel::Stable,
            channel_bump: None,
            assets: Vec::new(),
            github_release: None,
            depends_on: Vec::new(),
        }
    }

    #[test]
    fn plans_new_and_existing_changelog_edits() {
        let root = temporary_root();
        fs::create_dir_all(root.join("package"))
            .expect("package fixture directory must be created");
        let root = camino::Utf8Path::from_path(&root)
            .expect("temporary test directory must be valid UTF-8");
        let package = package_config();

        let new_edit = plan_changelog_edit(
            root,
            &package,
            &PackageId::new("app"),
            "## v1.0.0",
            "1.0.0",
            false,
        )
        .expect("new changelog edit must be planned");
        assert_eq!(
            new_edit.path,
            camino::Utf8Path::new("package").join("CHANGELOG.md")
        );
        assert_eq!(new_edit.expected, FileEditExpectation::Missing);
        assert_eq!(new_edit.new_content, "# Changelog\n\n## v1.0.0\n");

        fs::write(root.join("package/CHANGELOG.md"), "# Changelog\n")
            .expect("existing changelog fixture must be written");
        let existing_edit = plan_changelog_edit(
            root,
            &package,
            &PackageId::new("app"),
            "## v1.0.0",
            "1.0.0",
            false,
        )
        .expect("existing changelog edit must be planned");
        assert!(matches!(
            existing_edit.expected,
            FileEditExpectation::Existing { .. }
        ));
        assert_eq!(existing_edit.new_content, "# Changelog\n\n## v1.0.0\n");
        fs::remove_dir_all(root).expect("release apply fixture must be removed");
    }
}
