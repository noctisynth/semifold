use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use camino::Utf8PathBuf;
use semifold_core::{
    ChangesetId, ChangesetReference, ConfigSyncPlan, ConfigSyncPlanner, ConfiguredPackage,
    EcosystemId, PackageId,
};
use semifold_resolver::{changeset::Changeset, config::Config};

use crate::{
    discovery::{PackageDiscoveryError, PackageDiscoveryService},
    package_path::{PackagePathError, normalize_package_path},
};

/// Resolver selection and deletion safety for one config sync invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSyncScope {
    resolvers: Vec<EcosystemId>,
    pub is_complete: bool,
}

pub fn config_sync_scope(
    config: &Config,
    requested: &[EcosystemId],
) -> Result<ConfigSyncScope, ConfigSyncPlanningError> {
    let mut enabled = config.resolver.keys().cloned().collect::<Vec<_>>();
    enabled.extend(config.plugins.keys().cloned());
    enabled.sort();
    enabled.dedup();
    let resolvers = if requested.is_empty() {
        enabled.clone()
    } else {
        let mut requested = requested.to_vec();
        requested.sort();
        requested.dedup();
        requested
    };
    for resolver in &resolvers {
        if !config.resolver.contains_key(resolver) && !config.plugins.contains_key(resolver) {
            return Err(ConfigSyncPlanningError::ResolverNotEnabled {
                resolver: resolver.clone(),
            });
        }
    }

    Ok(ConfigSyncScope {
        is_complete: resolvers == enabled,
        resolvers,
    })
}

/// Builds one config sync plan without editing the configuration document.
pub fn plan_config_sync(
    project_root: &Path,
    config_path: &Path,
    config: &Config,
    changesets: &[Changeset],
    scope: &ConfigSyncScope,
    prune_missing: bool,
) -> Result<ConfigSyncPlan, ConfigSyncPlanningError> {
    if prune_missing && !scope.is_complete {
        return Err(ConfigSyncPlanningError::IncompletePrune);
    }
    let selected = scope.resolvers.iter().cloned().collect::<BTreeSet<_>>();
    let configured = config
        .packages
        .iter()
        .filter(|(_, package)| selected.contains(&package.resolver))
        .map(|(id, package)| {
            let path = normalize_package_path(project_root, &package.path).map_err(|source| {
                ConfigSyncPlanningError::ConfiguredPackagePath {
                    package: PackageId::new(id),
                    source,
                }
            })?;
            Ok(ConfiguredPackage {
                id: PackageId::new(id),
                ecosystem: package.resolver.clone(),
                path,
            })
        })
        .collect::<Result<Vec<_>, ConfigSyncPlanningError>>()?;
    let discovery = PackageDiscoveryService::from_config(project_root, config)
        .map_err(|source| ConfigSyncPlanningError::Discovery(Box::new(source)))?
        .discover(project_root, &scope.resolvers)
        .map_err(|source| ConfigSyncPlanningError::Discovery(Box::new(source)))?;
    let changesets = changesets
        .iter()
        .map(|changeset| ChangesetReference {
            changeset: ChangesetId::new(&changeset.name),
            packages: changeset
                .packages
                .iter()
                .map(|package| PackageId::new(&package.name))
                .collect(),
        })
        .collect::<Vec<_>>();
    let config_path = Utf8PathBuf::from_path_buf(config_path.to_path_buf())
        .map_err(|path| ConfigSyncPlanningError::NonUtf8ConfigPath { path })?;

    let mut plan =
        ConfigSyncPlanner::plan(config_path, &configured, &discovery.packages, &changesets);
    plan.prune_missing = prune_missing;
    Ok(plan)
}

#[derive(Debug)]
pub enum ConfigSyncPlanningError {
    IncompletePrune,
    ResolverNotEnabled {
        resolver: EcosystemId,
    },
    NonUtf8ConfigPath {
        path: PathBuf,
    },
    ConfiguredPackagePath {
        package: PackageId,
        source: PackagePathError,
    },
    Discovery(Box<PackageDiscoveryError>),
}

impl fmt::Display for ConfigSyncPlanningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompletePrune => {
                formatter.write_str("pruning requires a complete resolver scan")
            }
            Self::ResolverNotEnabled { resolver } => {
                write!(
                    formatter,
                    "resolver {resolver} is not enabled in the configuration"
                )
            }
            Self::NonUtf8ConfigPath { path } => {
                write!(
                    formatter,
                    "config path is not valid UTF-8: {}",
                    path.display()
                )
            }
            Self::ConfiguredPackagePath { package, source } => {
                write!(
                    formatter,
                    "configured package {package} has an invalid path: {source}"
                )
            }
            Self::Discovery(source) => source.fmt(formatter),
        }
    }
}

impl Error for ConfigSyncPlanningError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ConfiguredPackagePath { source, .. } => Some(source),
            Self::Discovery(source) => Some(source.as_ref()),
            Self::IncompletePrune
            | Self::ResolverNotEnabled { .. }
            | Self::NonUtf8ConfigPath { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use semifold_core::{ConfigSyncWarning, PackageRename};
    use semifold_resolver::{
        changeset::BumpLevel,
        config::{BranchesConfig, PackageConfig, PreCheckConfig, ReleaseChannel, ResolverConfig},
        resolver::ResolverType,
    };

    use super::*;

    struct TemporaryRoot(PathBuf);

    impl TemporaryRoot {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "semifold-config-sync-plan-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(path.join("crates/app")).unwrap();
            Self(path)
        }
    }

    impl Drop for TemporaryRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn package(path: &str, resolver: ResolverType) -> PackageConfig {
        PackageConfig {
            path: path.into(),
            resolver: resolver.into(),
            publish: None,
            channel: ReleaseChannel::Stable,
            channel_bump: None,
            assets: vec![],
            github_release: None,
            depends_on: vec![],
        }
    }

    fn resolver_config() -> ResolverConfig {
        ResolverConfig {
            pre_check: Some(PreCheckConfig::Http {
                url: String::new(),
                extra_headers: BTreeMap::new(),
                retry: Vec::new(),
            }),
            prepublish: vec![],
            publish: vec![],
            post_version: vec![],
        }
    }

    #[test]
    fn bridges_config_discovery_and_changesets_into_the_core_plan() {
        let root = TemporaryRoot::new();
        fs::write(
            root.0.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/app\"]\n",
        )
        .unwrap();
        fs::write(
            root.0.join("crates/app/Cargo.toml"),
            "[package]\nname = \"new-name\"\nversion = \"1.0.0\"\n",
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
                    "old-name".to_string(),
                    package("./crates/temp/../app", ResolverType::Rust),
                ),
                (
                    "ignored-node".to_string(),
                    package("packages/missing", ResolverType::Nodejs),
                ),
            ]),
            plugins: BTreeMap::new(),
            resolver: BTreeMap::from([(EcosystemId::RUST, resolver_config())]),
        };
        let mut changeset = Changeset::new("pending".to_string(), &root.0);
        changeset.add_package("old-name".to_string(), BumpLevel::Patch, None);

        let scope = config_sync_scope(&config, &[EcosystemId::RUST, EcosystemId::RUST]).unwrap();
        assert_eq!(scope.resolvers, [EcosystemId::RUST]);
        assert!(scope.is_complete);

        let plan = plan_config_sync(
            &root.0,
            &root.0.join(".changes/config.toml"),
            &config,
            &[changeset],
            &scope,
            false,
        )
        .unwrap();

        assert_eq!(
            plan.renamed,
            [PackageRename {
                from: PackageId::new("old-name"),
                to: PackageId::new("new-name"),
                ecosystem: semifold_core::EcosystemId::RUST,
                path: Utf8PathBuf::from("crates/app"),
            }]
        );
        assert_eq!(
            plan.warnings,
            [ConfigSyncWarning::ChangesetReferencesRenamedPackage {
                changeset: ChangesetId::new("pending"),
                from: PackageId::new("old-name"),
                to: PackageId::new("new-name"),
            }]
        );
        assert!(plan.missing.is_empty());
        assert!(plan.added.is_empty());
        assert!(plan.conflicts.is_empty());
    }
}
