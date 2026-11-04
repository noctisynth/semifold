#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use minijinja::{Environment, UndefinedBehavior, context};
pub use semifold_core::{
    ChangelogContext, ChangesetContext, DependencyUpdateContext, PackageChangesetContext,
};
use semifold_core::{
    ChangesetId, CommitContext, PackageId, PullRequestContext, ReleasePackageContext,
    RepositoryContext,
};
use semifold_resolver::{changeset, config::ChangelogConfig, error::ResolveError};
use serde::Serialize;

pub mod github;
pub mod types;
pub mod utils;

pub struct GeneratedChangelog {
    pub content: String,
    pub requires_marker: bool,
    pub remote_metadata_failed: bool,
    pub github_failures: Vec<github::GitHubFailure>,
}

pub struct CollectedChangelogContext<'release> {
    pub context: ChangelogContext<'release>,
    pub remote_metadata_failed: bool,
    pub github_failures: Vec<github::GitHubFailure>,
}

pub struct ChangelogSource<'a> {
    pub repo_root: &'a Path,
    pub tags: &'a BTreeMap<String, String>,
    pub repository: Option<&'a RepositoryContext>,
}

const RELEASE_TEMPLATE_NAME: &str = "changelog";
const CHANGESET_TEMPLATE_NAME: &str = "changeset";
pub const RELEASE_MARKER_PREFIX: &str = "<!-- semifold:release version=";
pub const RELEASE_MARKER_END: &str = "<!-- semifold:release:end -->";

const DEFAULT_CHANGESET_TEMPLATE: &str = concat!(
    "- ",
    "{% if changeset.commit and changeset.commit.web_url %}",
    "[`{{ changeset.commit.short_sha }}`]({{ changeset.commit.web_url }}): ",
    "{% endif %}",
    "{{ changeset.summary_paragraphs[0] | join(\" \") }}",
    "{% if changeset.pull_request %}",
    " {% if changeset.pull_request.web_url %}",
    "([#{{ changeset.pull_request.number }}]({{ changeset.pull_request.web_url }})",
    "{% else %}",
    "(#{{ changeset.pull_request.number }}",
    "{% endif %}",
    "{% if changeset.pull_request.author %} by @{{ changeset.pull_request.author }}{% endif %})",
    "{% endif %}",
    "{% for paragraph in changeset.summary_paragraphs %}",
    "{% if not loop.first %}\n\n    {{ paragraph | join(\" \") }}{% endif %}",
    "{% endfor %}",
    "{% if changeset.summary_paragraphs | length > 1 %}\n{% endif %}",
);

const DEFAULT_RELEASE_TEMPLATE: &str = concat!(
    "## v{{ package.next_version }}\n\n",
    "{% for section in sections %}",
    "### {{ section.name }}\n\n",
    "{% for entry in section.entries %}",
    "{{ entry.content }}{% if not loop.last %}\n{% endif %}",
    "{% endfor %}",
    "{% if not loop.last or dependency_updates %}\n\n{% endif %}",
    "{% endfor %}",
    "{% if dependency_updates %}",
    "### Dependencies\n\n",
    "{% for dependency in dependency_updates %}",
    "- Update {{ dependency.package }} to {{ dependency.next_version }}.",
    "{% if not loop.last %}\n{% endif %}",
    "{% endfor %}",
    "{% endif %}",
);

#[must_use]
pub fn default_changelog_config() -> ChangelogConfig {
    ChangelogConfig {
        template: Some(DEFAULT_RELEASE_TEMPLATE.to_string()),
        changeset_template: Some(DEFAULT_CHANGESET_TEMPLATE.to_string()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangelogTemplateKind {
    Release,
    Changeset,
}

impl fmt::Display for ChangelogTemplateKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Release => formatter.write_str("release"),
            Self::Changeset => formatter.write_str("changeset"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ChangelogRenderError {
    #[error("failed to compile {kind} changelog template for package {package}")]
    Compile {
        package: PackageId,
        kind: ChangelogTemplateKind,
        #[source]
        source: minijinja::Error,
    },
    #[error("failed to render changeset {changeset} for package {package}")]
    RenderChangeset {
        package: PackageId,
        changeset: ChangesetId,
        #[source]
        source: minijinja::Error,
    },
    #[error("failed to render changelog for package {package}")]
    RenderRelease {
        package: PackageId,
        #[source]
        source: minijinja::Error,
    },
    #[error("changeset {changeset} for package {package} rendered empty content")]
    EmptyChangeset {
        package: PackageId,
        changeset: ChangesetId,
    },
    #[error("changelog for package {package} rendered empty content")]
    EmptyRelease { package: PackageId },
    #[error("changelog for package {package} contains a reserved Semifold release marker")]
    ReservedMarker { package: PackageId },
}

#[derive(Debug, thiserror::Error)]
pub enum GenerateChangelogError {
    #[error(transparent)]
    Collect(#[from] ResolveError),
    #[error(transparent)]
    Render(#[from] ChangelogRenderError),
}

#[derive(Serialize)]
struct RenderedChangeset<'a> {
    section: &'a str,
    changeset: &'a ChangesetContext,
    content: String,
}

#[derive(Serialize)]
struct RenderedSection<'a> {
    name: &'a str,
    entries: Vec<RenderedChangeset<'a>>,
}

pub struct ChangelogRenderer {
    environment: Environment<'static>,
    customized: bool,
}

impl ChangelogRenderer {
    pub fn new(
        config: &ChangelogConfig,
        package: &PackageId,
    ) -> Result<Self, ChangelogRenderError> {
        let mut environment = Environment::new();
        environment.set_undefined_behavior(UndefinedBehavior::Strict);
        environment
            .add_template_owned(
                RELEASE_TEMPLATE_NAME.to_string(),
                config
                    .template
                    .as_deref()
                    .unwrap_or(DEFAULT_RELEASE_TEMPLATE)
                    .to_string(),
            )
            .map_err(|source| ChangelogRenderError::Compile {
                package: package.clone(),
                kind: ChangelogTemplateKind::Release,
                source,
            })?;
        environment
            .add_template_owned(
                CHANGESET_TEMPLATE_NAME.to_string(),
                config
                    .changeset_template
                    .as_deref()
                    .unwrap_or(DEFAULT_CHANGESET_TEMPLATE)
                    .to_string(),
            )
            .map_err(|source| ChangelogRenderError::Compile {
                package: package.clone(),
                kind: ChangelogTemplateKind::Changeset,
                source,
            })?;
        Ok(Self {
            environment,
            customized: config.template.is_some() || config.changeset_template.is_some(),
        })
    }

    pub fn render(&self, changelog: &ChangelogContext<'_>) -> Result<String, ChangelogRenderError> {
        let package = &changelog.package.package.id;
        let changeset_template = self
            .environment
            .get_template(CHANGESET_TEMPLATE_NAME)
            .map_err(|source| ChangelogRenderError::Compile {
                package: package.clone(),
                kind: ChangelogTemplateKind::Changeset,
                source,
            })?;
        let mut sections = BTreeMap::<&str, Vec<RenderedChangeset<'_>>>::new();
        for item in &changelog.changesets {
            let content = changeset_template
                .render(context! {
                    release => changelog.package.release,
                    package => &changelog.package.package,
                    section => &item.section,
                    changeset => &item.changeset,
                })
                .map_err(|source| ChangelogRenderError::RenderChangeset {
                    package: package.clone(),
                    changeset: item.changeset.id.clone(),
                    source,
                })?;
            if content.trim().is_empty() {
                return Err(ChangelogRenderError::EmptyChangeset {
                    package: package.clone(),
                    changeset: item.changeset.id.clone(),
                });
            }
            sections
                .entry(&item.section)
                .or_default()
                .push(RenderedChangeset {
                    section: &item.section,
                    changeset: &item.changeset,
                    content,
                });
        }
        let sections = sections
            .into_iter()
            .map(|(name, entries)| RenderedSection { name, entries })
            .collect::<Vec<_>>();
        let release_template = self
            .environment
            .get_template(RELEASE_TEMPLATE_NAME)
            .map_err(|source| ChangelogRenderError::Compile {
                package: package.clone(),
                kind: ChangelogTemplateKind::Release,
                source,
            })?;
        let content = release_template
            .render(context! {
                release => changelog.package.release,
                package => &changelog.package.package,
                changesets => &changelog.changesets,
                dependency_updates => &changelog.dependency_updates,
                sections => &sections,
            })
            .map_err(|source| ChangelogRenderError::RenderRelease {
                package: package.clone(),
                source,
            })?;
        if content.trim().is_empty() {
            return Err(ChangelogRenderError::EmptyRelease {
                package: package.clone(),
            });
        }
        if content.contains("<!-- semifold:release") {
            return Err(ChangelogRenderError::ReservedMarker {
                package: package.clone(),
            });
        }
        Ok(content)
    }

    #[must_use]
    pub const fn is_customized(&self) -> bool {
        self.customized
    }
}

pub async fn collect_changelog_context<'release>(
    source: &ChangelogSource<'_>,
    repo: &gix::Repository,
    changesets: &[changeset::Changeset],
    package: ReleasePackageContext<'release>,
    dependency_updates: Vec<DependencyUpdateContext>,
    collect_remote_metadata: bool,
) -> Result<CollectedChangelogContext<'release>, ResolveError> {
    let mut collected_changesets = Vec::new();
    let mut remote_metadata_failed = false;
    let mut github_failures = Vec::new();

    for changeset in changesets {
        let Some(changed_package) = changeset
            .packages
            .iter()
            .find(|changed| changed.name == package.package.id.as_str())
        else {
            continue;
        };
        let changeset_path = changeset
            .path
            .as_ref()
            .ok_or(ResolveError::InvalidChangeset {
                path: PathBuf::new(),
                reason: "Changeset is missing its source path".to_string(),
            })?;
        let rel_path = pathdiff::diff_paths(changeset_path, source.repo_root).ok_or(
            ResolveError::InvalidChangeset {
                path: changeset_path.to_path_buf(),
                reason: "Changeset path is not under repo root".to_string(),
            },
        )?;
        let commit_info = utils::find_first_commit_for_path(repo, &rel_path)?;
        let pr_info = if collect_remote_metadata
            && let Some(repository) = source.repository
            && let Some(commit_info) = commit_info.as_ref()
        {
            match utils::query_pr_for_commit(
                repository.owner.as_str(),
                repository.name.as_str(),
                commit_info,
            )
            .await
            {
                Ok(pr_info) => pr_info,
                Err(error) => {
                    remote_metadata_failed = true;
                    github_failures.push(error);
                    None
                }
            }
        } else {
            None
        };

        let commit = commit_info.map(|commit| {
            let sha = commit.oid.to_string();
            let short_sha = sha.chars().take(7).collect::<String>();
            let web_url = source
                .repository
                .map(|repository| format!("{}/commit/{sha}", repository.web_url));
            CommitContext {
                sha,
                short_sha,
                author: commit.author,
                web_url,
            }
        });
        let pull_request = pr_info.map(|pull_request| PullRequestContext {
            number: pull_request.number,
            author: pull_request.author,
            web_url: pull_request.url,
        });
        let section = changed_package
            .tag
            .as_ref()
            .and_then(|tag| source.tags.get(tag))
            .cloned()
            .unwrap_or_else(|| "Changes".to_string());
        collected_changesets.push(PackageChangesetContext {
            changeset: ChangesetContext {
                id: ChangesetId::new(&changeset.name),
                summary: changeset.summary.clone(),
                summary_paragraphs: summary_paragraphs(&changeset.summary),
                commit,
                pull_request,
            },
            section,
        });
    }
    collected_changesets.sort_by(|left, right| left.changeset.id.cmp(&right.changeset.id));

    Ok(CollectedChangelogContext {
        context: ChangelogContext {
            package,
            changesets: collected_changesets,
            dependency_updates,
        },
        remote_metadata_failed,
        github_failures,
    })
}

pub async fn generate_changelog<'release>(
    renderer: &ChangelogRenderer,
    source: &ChangelogSource<'_>,
    repo: &gix::Repository,
    changesets: &[changeset::Changeset],
    package: ReleasePackageContext<'release>,
    dependency_updates: Vec<DependencyUpdateContext>,
    collect_remote_metadata: bool,
) -> Result<GeneratedChangelog, GenerateChangelogError> {
    let collected = collect_changelog_context(
        source,
        repo,
        changesets,
        package,
        dependency_updates,
        collect_remote_metadata,
    )
    .await?;
    Ok(GeneratedChangelog {
        content: renderer.render(&collected.context)?,
        requires_marker: renderer.is_customized(),
        remote_metadata_failed: collected.remote_metadata_failed,
        github_failures: collected.github_failures,
    })
}

fn summary_paragraphs(summary: &str) -> Vec<Vec<String>> {
    let mut paragraphs = Vec::<Vec<String>>::new();
    for line in summary.lines() {
        if line.trim().is_empty() {
            if paragraphs
                .last()
                .is_some_and(|paragraph| !paragraph.is_empty())
            {
                paragraphs.push(Vec::new());
            }
        } else if let Some(paragraph) = paragraphs.last_mut() {
            paragraph.push(line.to_string());
        } else {
            paragraphs.push(vec![line.to_string()]);
        }
    }
    while paragraphs.last().is_some_and(Vec::is_empty) {
        paragraphs.pop();
    }
    paragraphs
}

pub async fn read_latest_changelog<P: AsRef<Path>>(
    path: P,
) -> Result<types::Changelog, ResolveError> {
    let content = std::fs::read_to_string(path.as_ref())?;

    let mut lines = content.lines();

    if lines.next().map(|l| l.trim()) != Some("# Changelog") {
        return Err(ResolveError::InvalidChangelog {
            path: path.as_ref().to_path_buf(),
            reason: "Invalid changelog: missing `# Changelog` header".to_string(),
        });
    }

    if utils::validate_release_markers(path.as_ref(), &content)? {
        let mut version = None;
        let mut body = Vec::new();
        for line in content.lines().skip(1) {
            let trimmed = line.trim();
            if version.is_none() {
                if let Some(marker_version) = utils::release_marker_version(trimmed) {
                    version = Some(format!("v{marker_version}"));
                }
            } else if trimmed == RELEASE_MARKER_END {
                let parsed_version = version.clone().ok_or(ResolveError::InvalidChangelog {
                    path: path.as_ref().to_path_buf(),
                    reason: "Release end marker has no matching version".to_string(),
                })?;
                return Ok(types::Changelog {
                    version: parsed_version,
                    body: body.join("\n").trim().to_string(),
                });
            } else {
                body.push(line);
            }
        }
        return Err(ResolveError::InvalidChangelog {
            path: path.as_ref().to_path_buf(),
            reason: "No complete Semifold release marker found".to_string(),
        });
    }

    let mut version: Option<String> = None;
    let mut body = String::new();
    let mut in_latest = false;

    for line in content.lines().skip(1) {
        let trimmed = line.trim();

        if version.is_none() {
            if let Some(rest) = trimmed.strip_prefix("## ") {
                version = Some(rest.to_string());
                in_latest = true;

                body.push_str(line);
                body.push('\n');
                continue;
            }
        } else if in_latest && trimmed.starts_with("## ") {
            break;
        } else if in_latest {
            body.push_str(line);
            body.push('\n');
        }
    }

    let version = version.ok_or(ResolveError::InvalidChangelog {
        path: path.as_ref().to_path_buf(),
        reason: "No version header found".to_string(),
    })?;

    Ok(types::Changelog {
        version,
        body: body.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, time::SystemTime};

    use semifold_core::{
        BumpLevel, ChangesetId, CommitContext, DependencyUpdateContext, EcosystemId, PackageId,
        PackageRelease, PackageSnapshot, PullRequestContext, ReleaseContext, ReleasePackageContext,
        ReleasePlan, ReleaseReason, VersionMap, VersionSource,
    };
    use semifold_resolver::config::ChangelogConfig;
    use semver::Version;

    use super::{
        ChangelogContext, ChangelogRenderError, ChangelogRenderer, ChangesetContext,
        PackageChangesetContext, read_latest_changelog, summary_paragraphs,
        utils::render_changelog,
    };

    fn release_context() -> ReleaseContext {
        let id = PackageId::new("package");
        let release = PackageRelease {
            id: id.clone(),
            ecosystem: EcosystemId::RUST,
            current_version: Version::new(0, 9, 0),
            next_version: Version::new(1, 0, 0),
            bump: BumpLevel::Major,
            reasons: vec![ReleaseReason::Changeset {
                changeset: ChangesetId::new("release"),
            }],
        };
        ReleaseContext::from_plan(
            &ReleasePlan::new(
                vec![release],
                VersionMap::from([(id.clone(), Version::new(1, 0, 0))]),
                vec![id],
                vec![ChangesetId::new("release")],
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        )
    }

    fn package_context(release: &ReleaseContext) -> ReleasePackageContext<'_> {
        ReleasePackageContext::from_snapshot(
            release,
            &PackageSnapshot {
                id: PackageId::new("package"),
                manifest_name: "package".to_string(),
                version: Version::new(0, 9, 0),
                version_source: VersionSource::PackageManifest,
                ecosystem: EcosystemId::RUST,
                path: "crates/package".into(),
                publishable: true,
                dependencies: Vec::new(),
            },
        )
        .unwrap()
    }

    fn changeset(id: &str, summary: &str) -> ChangesetContext {
        ChangesetContext {
            id: ChangesetId::new(id),
            summary: summary.to_string(),
            summary_paragraphs: summary_paragraphs(summary),
            commit: None,
            pull_request: None,
        }
    }

    fn render_default(context: &ChangelogContext<'_>) -> String {
        ChangelogRenderer::new(&ChangelogConfig::default(), &PackageId::new("package"))
            .unwrap()
            .render(context)
            .unwrap()
    }

    #[test]
    fn formats_an_immutable_context_without_external_clients() {
        let release = release_context();
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![PackageChangesetContext {
                changeset: changeset("release", "Add release planning"),
                section: "Changes".to_string(),
            }],
            dependency_updates: vec![DependencyUpdateContext {
                package: PackageId::new("core"),
                next_version: Version::new(1, 0, 0),
            }],
        };

        assert_eq!(
            render_default(&context),
            "## v1.0.0\n\n### Changes\n\n- Add release planning\n\n### Dependencies\n\n- Update core to 1.0.0."
        );
    }

    #[test]
    fn explicit_init_defaults_match_runtime_fallback_templates() {
        let release = release_context();
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![PackageChangesetContext {
                changeset: changeset("release", "Add release planning"),
                section: "Changes".to_string(),
            }],
            dependency_updates: vec![],
        };
        let fallback = render_default(&context);
        let explicit = ChangelogRenderer::new(
            &super::default_changelog_config(),
            &PackageId::new("package"),
        )
        .unwrap();

        assert_eq!(explicit.render(&context).unwrap(), fallback);
        assert!(explicit.is_customized());
    }

    #[test]
    fn formats_multiline_changesets_as_separate_list_item_paragraphs() {
        let release = release_context();
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![
                PackageChangesetContext {
                    changeset: changeset("first", "First line\n\nSecond line\nThird line"),
                    section: "Changes".to_string(),
                },
                PackageChangesetContext {
                    changeset: changeset("second", "Another changeset"),
                    section: "Changes".to_string(),
                },
            ],
            dependency_updates: vec![],
        };

        assert_eq!(
            render_default(&context),
            "## v1.0.0\n\n### Changes\n\n- First line\n\n    Second line Third line\n\n- Another changeset"
        );
    }

    #[test]
    fn ignores_source_blank_lines_without_emitting_whitespace_only_paragraphs() {
        let release = release_context();
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![PackageChangesetContext {
                changeset: changeset(
                    "realistic-multiline",
                    "Keep resume item columns within predictable bounds\n\nThe default template now gives job titles, organizations, and dates independent grid columns so\nlong content wraps without displacing adjacent fields. Linked titles no longer include trailing\nunderline space, and a complete Chinese sample covers long-title wrapping and all resume sections.",
                ),
                section: "Changes".to_string(),
            }],
            dependency_updates: vec![],
        };

        assert_eq!(
            render_default(&context),
            "## v1.0.0\n\n### Changes\n\n- Keep resume item columns within predictable bounds\n\n    The default template now gives job titles, organizations, and dates independent grid columns so long content wraps without displacing adjacent fields. Linked titles no longer include trailing underline space, and a complete Chinese sample covers long-title wrapping and all resume sections.\n"
        );
    }

    #[test]
    fn attaches_metadata_to_the_first_line_of_a_multiline_changeset() {
        let mut changeset = changeset("multiline", "First line\r\nSecond line\r\nThird line");
        changeset.commit = Some(CommitContext {
            sha: "1234567890abcdef".to_string(),
            short_sha: "1234567".to_string(),
            author: Some("commit-author".to_string()),
            web_url: Some(
                "https://github.com/semifold/semifold/commit/1234567890abcdef".to_string(),
            ),
        });
        changeset.pull_request = Some(PullRequestContext {
            number: 42,
            author: Some("author".to_string()),
            web_url: Some("https://github.com/semifold/semifold/pull/42".to_string()),
        });

        let release = release_context();
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![PackageChangesetContext {
                changeset,
                section: "Changes".to_string(),
            }],
            dependency_updates: vec![],
        };
        assert_eq!(
            render_default(&context),
            "## v1.0.0\n\n### Changes\n\n- [`1234567`](https://github.com/semifold/semifold/commit/1234567890abcdef): First line Second line Third line ([#42](https://github.com/semifold/semifold/pull/42) by @author)"
        );
    }

    #[test]
    fn renders_a_new_or_existing_changelog_without_writing() {
        assert_eq!(
            render_changelog(
                Path::new("CHANGELOG.md"),
                None,
                "## v1.0.0\n\n### Changes\n\n- Add",
                "1.0.0",
                false,
            )
            .unwrap(),
            "# Changelog\n\n## v1.0.0\n\n### Changes\n\n- Add\n"
        );
        assert_eq!(
            render_changelog(
                Path::new("CHANGELOG.md"),
                Some("# Changelog\n\n## v0.1.0\n\n- Old\n"),
                "## v1.0.0\n\n### Changes\n\n- Add",
                "1.0.0",
                false,
            )
            .unwrap(),
            "# Changelog\n\n## v1.0.0\n\n### Changes\n\n- Add\n\n## v0.1.0\n\n- Old\n"
        );
    }

    #[test]
    fn preserves_a_multiline_entries_trailing_blank_line_when_inserting() {
        assert_eq!(
            render_changelog(
                Path::new("CHANGELOG.md"),
                None,
                "## v1.0.0\n\n### Changes\n\n- First line\n\n    Second line\n",
                "1.0.0",
                false,
            )
            .unwrap(),
            "# Changelog\n\n## v1.0.0\n\n### Changes\n\n- First line\n\n    Second line\n\n"
        );
    }

    #[test]
    fn renders_custom_changeset_and_release_templates_with_all_metadata() {
        let release = release_context();
        let mut item = changeset("custom", "A custom summary");
        item.commit = Some(CommitContext {
            sha: "1234567890abcdef".to_string(),
            short_sha: "1234567".to_string(),
            author: Some("Commit Author".to_string()),
            web_url: Some("https://example.com/commit/1234567890abcdef".to_string()),
        });
        item.pull_request = Some(PullRequestContext {
            number: 42,
            author: Some("pr-author".to_string()),
            web_url: Some("https://example.com/pull/42".to_string()),
        });
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![PackageChangesetContext {
                changeset: item,
                section: "Features".to_string(),
            }],
            dependency_updates: vec![],
        };
        let renderer = ChangelogRenderer::new(
            &ChangelogConfig {
                template: Some(concat!(
                    "{{ package.next_version }}|",
                    "{% for section in sections %}{{ section.name }}:",
                    "{% for entry in section.entries %}{{ entry.content }}{% endfor %}",
                    "{% endfor %}",
                ).to_string()),
                changeset_template: Some(concat!(
                    "{{ changeset.commit.sha }}|{{ changeset.commit.short_sha }}|",
                    "{{ changeset.commit.author }}|{{ changeset.commit.web_url }}|",
                    "{{ changeset.pull_request.number }}|{{ changeset.pull_request.author }}|",
                    "{{ changeset.pull_request.web_url }}|{{ section }}|{{ changeset.summary }}",
                ).to_string()),
            },
            &PackageId::new("package"),
        )
        .unwrap();

        assert_eq!(
            renderer.render(&context).unwrap(),
            "1.0.0|Features:1234567890abcdef|1234567|Commit Author|https://example.com/commit/1234567890abcdef|42|pr-author|https://example.com/pull/42|Features|A custom summary"
        );
        assert!(renderer.is_customized());
    }

    #[test]
    fn custom_templates_use_strict_undefined_variables() {
        let release = release_context();
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![PackageChangesetContext {
                changeset: changeset("strict", "Strict"),
                section: "Changes".to_string(),
            }],
            dependency_updates: vec![],
        };
        let renderer = ChangelogRenderer::new(
            &ChangelogConfig {
                template: None,
                changeset_template: Some("{{ changeset.unknown }}".to_string()),
            },
            &PackageId::new("package"),
        )
        .unwrap();

        assert!(matches!(
            renderer.render(&context),
            Err(ChangelogRenderError::RenderChangeset { .. })
        ));
    }

    #[test]
    fn reports_which_configured_template_failed_to_compile() {
        let package = PackageId::new("package");
        let release_error = ChangelogRenderer::new(
            &ChangelogConfig {
                template: Some("{%".to_string()),
                changeset_template: None,
            },
            &package,
        );
        assert!(matches!(
            release_error,
            Err(ChangelogRenderError::Compile {
                kind: super::ChangelogTemplateKind::Release,
                ..
            })
        ));

        let changeset_error = ChangelogRenderer::new(
            &ChangelogConfig {
                template: None,
                changeset_template: Some("{%".to_string()),
            },
            &package,
        );
        assert!(matches!(
            changeset_error,
            Err(ChangelogRenderError::Compile {
                kind: super::ChangelogTemplateKind::Changeset,
                ..
            })
        ));
    }

    #[test]
    fn rejects_empty_changeset_and_release_template_results() {
        let release = release_context();
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![PackageChangesetContext {
                changeset: changeset("empty", "Summary"),
                section: "Changes".to_string(),
            }],
            dependency_updates: vec![],
        };
        let empty_changeset = ChangelogRenderer::new(
            &ChangelogConfig {
                template: None,
                changeset_template: Some(" \n".to_string()),
            },
            &PackageId::new("package"),
        )
        .unwrap();
        assert!(matches!(
            empty_changeset.render(&context),
            Err(ChangelogRenderError::EmptyChangeset { .. })
        ));

        let empty_release = ChangelogRenderer::new(
            &ChangelogConfig {
                template: Some(" \n".to_string()),
                changeset_template: None,
            },
            &PackageId::new("package"),
        )
        .unwrap();
        assert!(matches!(
            empty_release.render(&context),
            Err(ChangelogRenderError::EmptyRelease { .. })
        ));
    }

    #[test]
    fn custom_release_template_cannot_emit_reserved_markers() {
        let release = release_context();
        let context = ChangelogContext {
            package: package_context(&release),
            changesets: vec![PackageChangesetContext {
                changeset: changeset("marker", "Marker"),
                section: "Changes".to_string(),
            }],
            dependency_updates: vec![],
        };
        let renderer = ChangelogRenderer::new(
            &ChangelogConfig {
                template: Some("<!-- semifold:release version=1.0.0 -->".to_string()),
                changeset_template: None,
            },
            &PackageId::new("package"),
        )
        .unwrap();

        assert!(matches!(
            renderer.render(&context),
            Err(ChangelogRenderError::ReservedMarker { .. })
        ));
    }

    #[test]
    fn wraps_custom_entries_in_release_markers() {
        assert_eq!(
            render_changelog(
                Path::new("CHANGELOG.md"),
                None,
                "Custom release body",
                "1.0.0",
                true,
            )
            .unwrap(),
            concat!(
                "# Changelog\n\n",
                "<!-- semifold:release version=1.0.0 -->\n",
                "Custom release body\n",
                "<!-- semifold:release:end -->\n",
            )
        );
    }

    #[test]
    fn rejects_incomplete_existing_release_markers() {
        let result = render_changelog(
            Path::new("CHANGELOG.md"),
            Some("# Changelog\n\n<!-- semifold:release version=1.0.0 -->\nIncomplete\n"),
            "Next",
            "2.0.0",
            true,
        );

        assert!(result.is_err());
    }

    #[test]
    fn keeps_using_markers_after_custom_templates_are_removed() {
        let existing =
            render_changelog(Path::new("CHANGELOG.md"), None, "Custom", "1.0.0", true).unwrap();
        let updated = render_changelog(
            Path::new("CHANGELOG.md"),
            Some(&existing),
            "## v2.0.0",
            "2.0.0",
            false,
        )
        .unwrap();

        assert!(updated.starts_with(concat!(
            "# Changelog\n\n",
            "<!-- semifold:release version=2.0.0 -->\n",
            "## v2.0.0\n",
            "<!-- semifold:release:end -->\n",
        )));
    }

    #[tokio::test]
    async fn reads_marker_changelogs_and_falls_back_to_legacy_headings() {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "semifold-changelog-marker-{}-{nonce}.md",
            std::process::id()
        ));
        fs::write(
            &path,
            concat!(
                "# Changelog\n\n",
                "<!-- semifold:release version=2.0.0 -->\n",
                "Custom latest body\n",
                "<!-- semifold:release:end -->\n\n",
                "## v1.0.0\n\nLegacy body\n",
            ),
        )
        .unwrap();
        let marked = read_latest_changelog(&path).await.unwrap();
        assert_eq!(marked.version, "v2.0.0");
        assert_eq!(marked.body, "Custom latest body");

        fs::write(&path, "# Changelog\n\n## v1.0.0\n\nLegacy body\n").unwrap();
        let legacy = read_latest_changelog(&path).await.unwrap();
        assert_eq!(legacy.version, "v1.0.0");
        assert_eq!(legacy.body, "## v1.0.0\n\nLegacy body");
        fs::remove_file(path).unwrap();
    }
}
