use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use minijinja::{Environment, UndefinedBehavior, context};
use semifold_core::EcosystemId;
use semifold_resolver::{
    config::{
        self, BranchesConfig, CommandConfig, ConfigValidationError, PreCheckConfig, ResolverConfig,
        StdioType,
    },
    resolver::ResolverType,
};
use thiserror::Error;

use crate::{
    discovery::{PackageDiscoveryError, PackageDiscoveryService, ResolverRegistry},
    project::ProjectLocation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitWorkflowTemplates {
    pub release: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitOptions {
    pub target: Utf8PathBuf,
    pub resolvers: Vec<ResolverType>,
    pub tags: BTreeMap<String, String>,
    pub base_branch: String,
    pub release_branch: String,
    pub workflows: Option<InitWorkflowTemplates>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitFile {
    pub path: Utf8PathBuf,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitPlan {
    pub directories: Vec<Utf8PathBuf>,
    pub files: Vec<InitFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitReport {
    pub files: Vec<Utf8PathBuf>,
}

pub fn plan_init(
    location: &ProjectLocation,
    options: InitOptions,
) -> Result<InitPlan, InitPlanningError> {
    let resolvers = ResolverRegistry::normalize_selection(&options.resolvers);
    let ecosystems = resolvers
        .iter()
        .map(|resolver| resolver.ecosystem())
        .collect::<Vec<_>>();
    let discovery = PackageDiscoveryService::default()
        .discover(location.root.as_std_path(), &ecosystems)
        .map_err(|source| InitPlanningError::Discovery(Box::new(source)))?;
    let packages = discovery
        .default_package_configs()
        .map_err(|source| InitPlanningError::Discovery(Box::new(source)))?;
    let resolver = resolver_configs(&resolvers);
    let config = config::Config {
        branches: BranchesConfig {
            base: options.base_branch.clone(),
            release: options.release_branch,
        },
        release: Default::default(),
        tags: options.tags,
        changelog: semifold_changelog::default_changelog_config(),
        packages,
        plugins: BTreeMap::new(),
        resolver,
    };
    config
        .branches
        .validate_release_branch(&config.branches.release)
        .map_err(InitPlanningError::ConfigValidation)?;
    let config_path = options.target.join("config.toml");
    let config_content = toml_edit::ser::to_string_pretty(&config).map_err(|source| {
        InitPlanningError::ConfigSerialize {
            path: config_path.clone(),
            reason: source.to_string(),
        }
    })?;

    let mut directories = vec![options.target];
    let mut files = vec![InitFile {
        path: config_path,
        content: config_content,
    }];
    if let Some(workflows) = options.workflows {
        let workflow_dir = location.root.join(".github/workflows");
        files.push(InitFile {
            path: workflow_dir.join("semifold-ci.yaml"),
            content: render_workflow(&workflows.release, &options.base_branch, &resolvers)
                .map_err(InitPlanningError::WorkflowRender)?,
        });
        files.push(InitFile {
            path: workflow_dir.join("semifold-status.yaml"),
            content: render_workflow(&workflows.status, &options.base_branch, &resolvers)
                .map_err(InitPlanningError::WorkflowRender)?,
        });
        directories.push(workflow_dir);
    }
    directories.sort();
    directories.dedup();
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(InitPlan { directories, files })
}

fn render_workflow(
    template: &str,
    base_branch: &str,
    resolvers: &[ResolverType],
) -> Result<String, minijinja::Error> {
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment.render_str(
        template,
        context!(base_branch => base_branch, resolvers => minijinja::value::Serde(resolvers)),
    )
}

fn resolver_configs(resolvers: &[ResolverType]) -> BTreeMap<EcosystemId, ResolverConfig> {
    resolvers
        .iter()
        .copied()
        .map(|resolver| (resolver.ecosystem(), resolver_config(resolver)))
        .collect()
}

fn resolver_config(resolver: ResolverType) -> ResolverConfig {
    let retry = || vec![2, 5, 15, 30];
    match resolver {
        ResolverType::Rust => ResolverConfig {
            pre_check: Some(PreCheckConfig::Http {
                url: "https://crates.io/api/v1/crates/{{ package.name }}/{{ package.version }}"
                    .to_string(),
                extra_headers: BTreeMap::new(),
                retry: retry(),
            }),
            prepublish: vec![],
            publish: vec![command("cargo", &["publish"], None)],
            post_version: vec![command("cargo", &["generate-lockfile"], Some(true))],
        },
        ResolverType::Nodejs => ResolverConfig {
            pre_check: Some(PreCheckConfig::Http {
                url: "https://registry.npmjs.org/{{ package.name }}/{{ package.version }}"
                    .to_string(),
                extra_headers: BTreeMap::new(),
                retry: retry(),
            }),
            prepublish: vec![],
            publish: vec![command(
                "npm",
                &["publish", "--provenance", "--access", "public"],
                None,
            )],
            post_version: vec![],
        },
        ResolverType::Python => ResolverConfig {
            pre_check: Some(PreCheckConfig::Http {
                url: "https://pypi.org/pypi/{{ package.name }}/{{ package.version }}/json"
                    .to_string(),
                extra_headers: BTreeMap::new(),
                retry: retry(),
            }),
            prepublish: vec![],
            publish: vec![],
            post_version: vec![],
        },
        ResolverType::Cpp => ResolverConfig {
            pre_check: Some(PreCheckConfig::Http {
                url: String::new(),
                extra_headers: BTreeMap::new(),
                retry: retry(),
            }),
            prepublish: vec![],
            publish: vec![],
            post_version: vec![],
        },
    }
}

fn command(command: &str, args: &[&str], dry_run: Option<bool>) -> CommandConfig {
    CommandConfig {
        command: command.to_string(),
        args: Some(
            args.iter()
                .map(|argument| (*argument).to_string())
                .collect(),
        ),
        extra_env: BTreeMap::new(),
        stdout: StdioType::Inherit,
        stderr: StdioType::Inherit,
        dry_run,
    }
}

#[derive(Debug, Error)]
pub enum InitPlanningError {
    #[error("invalid branch configuration")]
    ConfigValidation(#[source] ConfigValidationError),
    #[error("package discovery failed during initialization: {0}")]
    Discovery(#[source] Box<PackageDiscoveryError>),
    #[error("failed to serialize initial configuration {path}: {reason}")]
    ConfigSerialize { path: Utf8PathBuf, reason: String },
    #[error("failed to render initialization workflow")]
    WorkflowRender(#[source] minijinja::Error),
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temporary_root() -> Utf8PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            Utf8PathBuf::from_path_buf(std::env::temp_dir().join(format!("semifold-init-{nonce}")))
                .unwrap();
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn plans_configuration_and_workflows_without_writing() {
        let root = temporary_root();
        fs::write(
            root.join("package.json"),
            r#"{"name":"example","version":"1.0.0"}"#,
        )
        .unwrap();
        let location = ProjectLocation {
            root: root.clone(),
            existing_config: None,
        };
        let target = root.join(".changes");
        let plan = plan_init(
            &location,
            InitOptions {
                target: target.clone(),
                resolvers: vec![ResolverType::Nodejs, ResolverType::Nodejs],
                tags: BTreeMap::new(),
                base_branch: "main".to_string(),
                release_branch: "release".to_string(),
                workflows: Some(InitWorkflowTemplates {
                    release: "base={{ base_branch }}{% for resolver in resolvers %} {{ resolver }}{% endfor %}".to_string(),
                    status: "{{ base_branch }}".to_string(),
                }),
            },
        )
        .unwrap();

        assert_eq!(
            plan.directories,
            vec![target.clone(), root.join(".github/workflows")]
        );
        assert_eq!(plan.files.len(), 3);
        assert!(!target.exists());
        let config = plan
            .files
            .iter()
            .find(|file| file.path == target.join("config.toml"))
            .unwrap();
        assert!(config.content.contains("[packages.example]"));
        assert!(config.content.contains("resolver = \"nodejs\""));
        assert!(config.content.contains("[changelog]"));
        assert!(config.content.contains("changeset-template"));
        assert!(config.content.contains("template = \"\"\""));
        assert!(!config.content.contains("User-Agent"));
        let parsed: config::Config = toml_edit::de::from_str(&config.content).unwrap();
        assert_eq!(
            parsed.changelog,
            semifold_changelog::default_changelog_config()
        );
        assert!(matches!(
            parsed
                .resolver
                .get(&EcosystemId::NODE)
                .and_then(|resolver| resolver.pre_check.as_ref()),
            Some(PreCheckConfig::Http { retry, .. }) if retry == &[2, 5, 15, 30]
        ));
        assert!(
            plan.files
                .iter()
                .any(|file| file.content == "base=main nodejs")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_matching_base_and_release_branches_without_writing() {
        let root = temporary_root();
        fs::write(
            root.join("package.json"),
            r#"{"name":"example","version":"1.0.0"}"#,
        )
        .unwrap();
        let location = ProjectLocation {
            root: root.clone(),
            existing_config: None,
        };
        let target = root.join(".changes");

        let result = plan_init(
            &location,
            InitOptions {
                target: target.clone(),
                resolvers: vec![ResolverType::Nodejs],
                tags: BTreeMap::new(),
                base_branch: "main".to_string(),
                release_branch: "main".to_string(),
                workflows: None,
            },
        );

        assert!(matches!(
            result,
            Err(InitPlanningError::ConfigValidation(
                ConfigValidationError::ReleaseBranchMatchesBase { branch }
            )) if branch == "main"
        ));
        assert!(!target.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plans_distinct_ids_for_matching_names_across_ecosystems() {
        let root = temporary_root();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"shared\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"name":"shared","version":"1.0.0"}"#,
        )
        .unwrap();
        let location = ProjectLocation {
            root: root.clone(),
            existing_config: None,
        };
        let target = root.join(".changes");

        let plan = plan_init(
            &location,
            InitOptions {
                target: target.clone(),
                resolvers: vec![ResolverType::Rust, ResolverType::Nodejs],
                tags: BTreeMap::new(),
                base_branch: "main".to_string(),
                release_branch: "release".to_string(),
                workflows: None,
            },
        )
        .unwrap();

        let config = plan
            .files
            .iter()
            .find(|file| file.path == target.join("config.toml"))
            .unwrap();
        let parsed: config::Config = toml_edit::de::from_str(&config.content).unwrap();
        assert_eq!(
            parsed
                .packages
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["nodejs-shared", "rust-shared"]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_workflow_template_returns_an_error() {
        let error = render_workflow("{{ missing }}", "main", &[]).unwrap_err();
        assert_eq!(error.kind(), minijinja::ErrorKind::UndefinedError);
    }

    #[test]
    fn rust_init_generates_lockfile_without_forcing_offline_mode() {
        let config = resolver_config(ResolverType::Rust);

        assert_eq!(config.post_version.len(), 1);
        assert_eq!(config.post_version[0].command, "cargo");
        assert_eq!(
            config.post_version[0].args.as_deref(),
            Some(["generate-lockfile".to_string()].as_slice())
        );
        assert_eq!(config.post_version[0].dry_run, Some(true));
        let Some(PreCheckConfig::Http {
            extra_headers,
            retry,
            ..
        }) = config.pre_check
        else {
            panic!("Rust init must generate an HTTP pre-check");
        };
        assert!(extra_headers.is_empty());
        assert_eq!(retry, vec![2, 5, 15, 30]);
    }
}
