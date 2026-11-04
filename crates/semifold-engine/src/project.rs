use std::path::{Path, PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use semifold_resolver::{config, error::ResolveError};
use thiserror::Error;

const CHANGESET_DIRECTORIES: [&str; 2] = [".changesets", ".changes"];
const CONFIG_FILES: [&str; 2] = ["config.toml", "config.json"];

#[derive(Debug)]
pub struct Project {
    pub root: Utf8PathBuf,
    pub changeset_dir: Utf8PathBuf,
    pub config_path: Utf8PathBuf,
    pub config: config::Config,
}

impl Project {
    pub fn load(location: ProjectLocation) -> Result<Self, ProjectLoadError> {
        let config_path = location.config_path()?.to_path_buf();
        let changeset_dir = config_path
            .parent()
            .map(Utf8PathBuf::from)
            .ok_or(ProjectLoadError::ChangesetDirectoryNotFound)?;
        let config = config::load_config(config_path.as_std_path()).map_err(|source| {
            ProjectLoadError::ConfigInvalid {
                path: config_path.clone(),
                source,
            }
        })?;
        Ok(Self {
            root: location.root,
            changeset_dir,
            config_path,
            config,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectLocation {
    pub root: Utf8PathBuf,
    pub existing_config: Option<Utf8PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectLocator {
    start: PathBuf,
    changeset_dir: Option<PathBuf>,
}

impl ProjectLocator {
    pub fn new(start: PathBuf, changeset_dir: Option<PathBuf>) -> Self {
        Self {
            start,
            changeset_dir,
        }
    }

    pub fn locate(&self) -> Result<ProjectLocation, ProjectLoadError> {
        ProjectLocation::discover_with_changeset_dir(&self.start, self.changeset_dir.as_deref())
    }

    pub fn load(&self) -> Result<Project, ProjectLoadError> {
        self.locate()?.load()
    }
}

impl ProjectLocation {
    pub fn discover(start: &Path) -> Result<Self, ProjectLoadError> {
        Self::discover_with_changeset_dir(start, None)
    }

    pub fn discover_with_changeset_dir(
        start: &Path,
        changeset_dir: Option<&Path>,
    ) -> Result<Self, ProjectLoadError> {
        let start = utf8_path(start)?;
        let repository =
            crate::git::discover_repository(start.as_std_path()).map_err(|source| {
                ProjectLoadError::RepositoryOpenFailed {
                    path: start.clone(),
                    source,
                }
            })?;
        let root = repository
            .workdir()
            .ok_or(ProjectLoadError::RepositoryNotFound)?;
        let root = utf8_path(root)?;
        let changeset_dir = changeset_dir.map_or_else(
            || discover_changeset_directory(root.as_std_path()),
            |path| {
                let path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    root.as_std_path().join(path)
                };
                path.is_dir().then_some(path)
            },
        );
        let existing_config = changeset_dir
            .as_deref()
            .and_then(discover_config)
            .map(|path| utf8_path(&path))
            .transpose()?;
        Ok(Self {
            root,
            existing_config,
        })
    }

    pub fn load(self) -> Result<Project, ProjectLoadError> {
        self.config_path()?;
        Project::load(self)
    }

    pub fn config_path(&self) -> Result<&Utf8Path, ProjectLoadError> {
        if let Some(config_path) = self.existing_config.as_deref() {
            return Ok(config_path);
        }
        if CHANGESET_DIRECTORIES
            .iter()
            .any(|directory| self.root.join(directory).is_dir())
        {
            Err(ProjectLoadError::ConfigNotFound)
        } else {
            Err(ProjectLoadError::ChangesetDirectoryNotFound)
        }
    }
}

#[derive(Debug, Error)]
pub enum ProjectLoadError {
    #[error("repository not found")]
    RepositoryNotFound,
    #[error("changeset directory not found")]
    ChangesetDirectoryNotFound,
    #[error("configuration not found")]
    ConfigNotFound,
    #[error("project path is not valid UTF-8: {path:?}")]
    NonUtf8Path { path: PathBuf },
    #[error("configuration at {path} is invalid")]
    ConfigInvalid {
        path: Utf8PathBuf,
        #[source]
        source: ResolveError,
    },
    #[error("failed to open repository from {path}: {source:#}")]
    RepositoryOpenFailed {
        path: Utf8PathBuf,
        #[source]
        source: gix::Error,
    },
}

impl ProjectLoadError {
    #[must_use]
    pub fn config_migration_may_help(&self) -> bool {
        matches!(
            self,
            Self::ConfigInvalid {
                source: ResolveError::InvalidConfig { .. },
                ..
            }
        )
    }
}

fn utf8_path(path: &Path) -> Result<Utf8PathBuf, ProjectLoadError> {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|path| ProjectLoadError::NonUtf8Path { path })
}

fn discover_changeset_directory(root: &Path) -> Option<PathBuf> {
    CHANGESET_DIRECTORIES
        .iter()
        .map(|directory| root.join(directory))
        .find(|path| path.is_dir())
}

fn discover_config(changeset_dir: &Path) -> Option<PathBuf> {
    CONFIG_FILES
        .iter()
        .map(|file| changeset_dir.join(file))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    fn fixture(name: &str) -> PathBuf {
        let temporary_root = std::env::temp_dir()
            .canonicalize()
            .expect("temporary directory can be canonicalized in tests");
        let root = temporary_root.join(format!(
            "semifold-project-{name}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join(".changes")).unwrap();
        gix::init(&root).unwrap();
        root
    }

    #[test]
    fn loads_a_complete_project_without_runtime_capabilities() {
        let root = fixture("complete");
        fs::write(
            root.join(".changes/config.toml"),
            "[branches]\nbase = \"main\"\nrelease = \"release\"\n\n[tags]\n\n[packages]\n\n[resolver]\n",
        )
        .unwrap();

        let project = ProjectLocation::discover(&root).unwrap().load().unwrap();

        assert_eq!(
            project.root.as_std_path().canonicalize().unwrap(),
            root.canonicalize().unwrap()
        );
        assert_eq!(project.changeset_dir, project.root.join(".changes"));
        assert_eq!(
            project.config_path,
            project.changeset_dir.join("config.toml")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn distinguishes_missing_changeset_directory_and_configuration() {
        let root = fixture("missing-config");
        assert!(matches!(
            ProjectLocation::discover(&root).unwrap().load(),
            Err(ProjectLoadError::ConfigNotFound)
        ));
        fs::remove_dir_all(root.join(".changes")).unwrap();
        assert!(matches!(
            ProjectLocation::discover(&root).unwrap().load(),
            Err(ProjectLoadError::ChangesetDirectoryNotFound)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovers_json_configuration_only_to_report_it_as_unsupported() {
        let root = fixture("json-config");
        fs::write(root.join(".changes/config.json"), "{}").unwrap();

        let result = ProjectLocation::discover(&root).unwrap().load();

        assert!(matches!(
            result,
            Err(ProjectLoadError::ConfigInvalid {
                source: ResolveError::UnsupportedConfigFormat { .. },
                ..
            })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recommends_migration_only_for_invalid_toml_contracts() {
        let path = Utf8PathBuf::from(".changes/config.toml");
        let invalid_toml = ProjectLoadError::ConfigInvalid {
            path: path.clone(),
            source: ResolveError::InvalidConfig {
                path: path.as_std_path().to_path_buf(),
                reason: "legacy shape".to_string(),
            },
        };
        let unsupported_json = ProjectLoadError::ConfigInvalid {
            path: Utf8PathBuf::from(".changes/config.json"),
            source: ResolveError::UnsupportedConfigFormat {
                path: PathBuf::from(".changes/config.json"),
            },
        };
        let unreadable = ProjectLoadError::ConfigInvalid {
            path,
            source: ResolveError::IoError(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "fixture is unreadable",
            )),
        };

        assert!(invalid_toml.config_migration_may_help());
        assert!(!unsupported_json.config_migration_may_help());
        assert!(!unreadable.config_migration_may_help());
        assert!(!ProjectLoadError::ConfigNotFound.config_migration_may_help());
    }

    #[cfg(unix)]
    #[test]
    fn reports_non_utf8_repository_paths_without_lossy_conversion() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let parent = fixture("non-utf8-parent");
        let root = parent.join(OsString::from_vec(vec![b'p', b'r', b'o', b'j', 0xff]));

        assert!(matches!(
            utf8_path(&root),
            Err(ProjectLoadError::NonUtf8Path { .. })
        ));
        fs::remove_dir_all(parent).unwrap();
    }
}
