use core::fmt;
use std::path::{Path, PathBuf};

use saphyr::{LoadableYamlNode, Mapping, Scalar, Yaml, YamlEmitter};
use serde::{Deserialize, Serialize};

use crate::{config::Config, error::ResolveError};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum BumpLevel {
    Major = 3,
    Minor = 2,
    Patch = 1,
    Unchanged = 0,
}

impl fmt::Display for BumpLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BumpLevel::Major => write!(f, "major"),
            BumpLevel::Minor => write!(f, "minor"),
            BumpLevel::Patch => write!(f, "patch"),
            BumpLevel::Unchanged => write!(f, "unchanged"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePackage {
    pub name: String,
    pub level: BumpLevel,
    pub tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Changeset {
    pub name: String,
    pub packages: Vec<ChangePackage>,
    pub summary: String,
    pub root_path: PathBuf,
    pub path: Option<PathBuf>,
}

impl Changeset {
    pub fn new(name: String, root_path: &Path) -> Self {
        Self {
            name,
            packages: Vec::new(),
            summary: String::new(),
            root_path: root_path.to_path_buf(),
            path: None,
        }
    }

    #[inline]
    pub fn add_package(&mut self, name: String, level: BumpLevel, tag: Option<String>) {
        self.packages.push(ChangePackage { name, level, tag })
    }

    pub fn add_packages(&mut self, packages: &[String], level: BumpLevel, tag: Option<String>) {
        packages.iter().for_each(|package| {
            self.add_package(package.clone(), level, tag.clone());
        })
    }

    pub fn summary(&mut self, summary: String) {
        self.summary = summary;
    }

    pub fn from_file(config: &Config, path: &PathBuf) -> Result<Self, ResolveError> {
        let changeset_str = std::fs::read_to_string(path)?;
        let separators = changeset_str
            .split_inclusive('\n')
            .scan(0, |offset, line| {
                let start = *offset;
                *offset += line.len();
                Some((start, *offset, line))
            })
            .filter(|(_, _, line)| line.trim_end_matches(['\r', '\n']) == "---")
            .map(|(start, end, _)| (start, end))
            .collect::<Vec<_>>();
        let (front_matter_start, summary_marker_start, summary_start) = match separators.as_slice()
        {
            [(summary_marker_start, summary_start)] if *summary_marker_start != 0 => {
                (0, *summary_marker_start, *summary_start)
            }
            [
                (opening_marker_start, front_matter_start),
                (summary_marker_start, summary_start),
            ] if *opening_marker_start == 0 => {
                (*front_matter_start, *summary_marker_start, *summary_start)
            }
            _ => {
                return Err(ResolveError::InvalidChangeset {
                    path: path.to_path_buf(),
                    reason: "Changeset must contain one front matter separator, with an optional opening `---` marker".to_string(),
                });
            }
        };

        let front_matter = &changeset_str[front_matter_start..summary_marker_start];
        let fm = Yaml::load_from_str(front_matter).map_err(|e| ResolveError::InvalidChangeset {
            path: path.to_path_buf(),
            reason: format!("Failed to parse changeset front matter: {e}"),
        })?;
        let packages_map = fm.first().and_then(|f| f.as_mapping());

        let mut packages = Vec::new();
        if let Some(map) = packages_map {
            map.into_iter().try_for_each(|(k, v)| {
                let name = k
                    .as_str()
                    .ok_or(ResolveError::InvalidChangeset {
                        path: path.to_path_buf(),
                        reason: format!("Failed to parse package name: {k:?}"),
                    })?
                    .to_string();
                if !config.packages.contains_key(&name) {
                    return Err(ResolveError::InvalidChangeset {
                        path: path.to_path_buf(),
                        reason: format!("Package {name} is not defined in config"),
                    });
                }

                let mark = v
                    .as_str()
                    .ok_or(ResolveError::InvalidChangeset {
                        path: path.to_path_buf(),
                        reason: format!("Failed to parse package mark: {v:?}"),
                    })?
                    .to_string();
                let mut mark = mark.split(':');
                let level = mark.next().ok_or(ResolveError::InvalidChangeset {
                    path: path.to_path_buf(),
                    reason: format!("Failed to parse package mark: {v:?}"),
                })?;
                let tag = mark.next().map(|s| s.to_string());
                if mark.next().is_some() {
                    return Err(ResolveError::InvalidChangeset {
                        path: path.to_path_buf(),
                        reason: format!("Package mark contains too many separators: {v:?}"),
                    });
                }
                if let Some(tag) = tag.as_ref()
                    && !config.tags.contains_key(tag)
                {
                    return Err(ResolveError::InvalidChangeset {
                        path: path.to_path_buf(),
                        reason: format!("Tag {tag} is not defined in config"),
                    });
                }
                let level = match level {
                    "major" => BumpLevel::Major,
                    "minor" => BumpLevel::Minor,
                    "patch" => BumpLevel::Patch,
                    _ => {
                        return Err(ResolveError::InvalidChangeset {
                            path: path.to_path_buf(),
                            reason: format!("Invalid bump level: {level}"),
                        });
                    }
                };
                packages.push(ChangePackage { name, level, tag });
                Ok(())
            })?;
        }
        if packages.is_empty() {
            return Err(ResolveError::InvalidChangeset {
                path: path.to_path_buf(),
                reason: "Changeset must contain at least one package".to_string(),
            });
        }

        let summary = changeset_str[summary_start..].trim().to_string();
        if summary.is_empty() {
            return Err(ResolveError::InvalidChangeset {
                path: path.to_path_buf(),
                reason: "Changeset summary must not be empty".to_string(),
            });
        }

        Ok(Self {
            name: path
                .file_stem()
                .ok_or(ResolveError::InvalidChangeset {
                    path: path.to_path_buf(),
                    reason: "Invalid changeset".to_string(),
                })?
                .to_string_lossy()
                .to_string(),
            packages,
            summary,
            root_path: path
                .parent()
                .ok_or(ResolveError::InvalidChangeset {
                    path: path.to_path_buf(),
                    reason: "Changeset path has no parent directory".to_string(),
                })?
                .to_path_buf(),
            path: Some(path.to_path_buf()),
        })
    }

    pub fn commit_to(&mut self, changeset_path: &Path) -> Result<(), ResolveError> {
        log::debug!("Commit changeset: {self:?}");

        let file_path = changeset_path.join(format!("{}.md", self.name));

        std::fs::write(&file_path, self.render()?)?;

        self.path = Some(file_path);

        Ok(())
    }

    pub fn render(&self) -> Result<String, ResolveError> {
        let file_path = self.root_path.join(format!("{}.md", self.name));

        let mut fm = String::new();
        let mut emitter = YamlEmitter::new(&mut fm);
        let mut fm_map = Mapping::new();
        for package in &self.packages {
            let mark = if let Some(tag) = &package.tag {
                format!("{}:{}", package.level, tag)
            } else {
                format!("{}", package.level)
            };

            fm_map.insert(
                Yaml::Value(Scalar::String(package.name.as_str().into())),
                Yaml::Value(Scalar::String(mark.into())),
            );
        }
        emitter
            .dump(&Yaml::Mapping(fm_map))
            .map_err(|e| ResolveError::ParseError {
                path: file_path.clone(),
                reason: e.to_string(),
            })?;

        Ok(format!("{fm}\n---\n\n{}\n", self.summary))
    }

    pub fn commit(&mut self) -> Result<(), ResolveError> {
        self.commit_to(&self.root_path.clone())
    }

    pub fn clean(&self) -> Result<(), ResolveError> {
        let file_path = self.root_path.join(format!("{}.md", self.name));
        std::fs::remove_file(file_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        config::{BranchesConfig, Config, PackageConfig, ReleaseChannel},
        error::ResolveError,
        resolver::ResolverType,
    };

    use super::{BumpLevel, Changeset};

    fn temp_dir(test_name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "semifold-resolver-{test_name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn config_with_packages(packages: &[&str]) -> Config {
        let packages = packages
            .iter()
            .map(|name| {
                (
                    (*name).to_string(),
                    PackageConfig {
                        path: PathBuf::from(name),
                        resolver: ResolverType::Rust.into(),
                        publish: None,
                        channel: ReleaseChannel::Stable,
                        channel_bump: None,
                        assets: vec![],
                        github_release: None,
                        depends_on: vec![],
                    },
                )
            })
            .collect();

        Config {
            branches: BranchesConfig {
                base: "main".to_string(),
                release: "release".to_string(),
            },
            release: Default::default(),
            tags: BTreeMap::new(),
            changelog: Default::default(),
            packages,
            plugins: BTreeMap::new(),
            resolver: BTreeMap::new(),
        }
    }

    #[test]
    fn writes_and_reads_a_changeset_with_tags() {
        let root = temp_dir("roundtrip");
        let mut config = config_with_packages(&["api", "web"]);
        config
            .tags
            .insert("feat".to_string(), "Features".to_string());
        let mut changeset = Changeset::new("add-api".to_string(), &root);
        changeset.add_package(
            "api".to_string(),
            BumpLevel::Minor,
            Some("feat".to_string()),
        );
        changeset.add_package("web".to_string(), BumpLevel::Patch, None);
        changeset.summary("Add the new API.\n\nIt supports batch requests.".to_string());

        changeset.commit().unwrap();

        let path = root.join("add-api.md");
        let parsed = Changeset::from_file(&config, &path).unwrap();

        assert_eq!(parsed.name, "add-api");
        assert_eq!(
            parsed.summary,
            "Add the new API.\n\nIt supports batch requests."
        );
        assert_eq!(parsed.packages.len(), 2);
        assert_eq!(parsed.packages[0].name, "api");
        assert_eq!(parsed.packages[0].level, BumpLevel::Minor);
        assert_eq!(parsed.packages[0].tag.as_deref(), Some("feat"));
        assert_eq!(parsed.packages[1].name, "web");
        assert_eq!(parsed.packages[1].level, BumpLevel::Patch);
        assert_eq!(parsed.packages[1].tag, None);

        parsed.clean().unwrap();
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_changesets_for_unknown_packages() {
        let root = temp_dir("unknown-package");
        let path = root.join("invalid.md");
        fs::write(&path, "---\nmissing: patch\n---\n\nUnknown package.\n").unwrap();

        let error = Changeset::from_file(&config_with_packages(&["api"]), &path).unwrap_err();

        assert!(matches!(error, ResolveError::InvalidChangeset { .. }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unknown_bump_levels() {
        let root = temp_dir("unknown-level");
        let path = root.join("invalid.md");
        fs::write(&path, "---\napi: breaking\n---\n\nUnknown level.\n").unwrap();

        let error = Changeset::from_file(&config_with_packages(&["api"]), &path).unwrap_err();

        assert!(matches!(error, ResolveError::InvalidChangeset { .. }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unknown_tags_and_extra_package_mark_separators() {
        let root = temp_dir("invalid-tags");
        let config = config_with_packages(&["api"]);
        for (name, mark) in [("unknown-tag", "patch:missing"), ("extra", "patch:a:b")] {
            let path = root.join(format!("{name}.md"));
            fs::write(&path, format!("---\napi: {mark}\n---\n\nSummary.\n")).unwrap();

            assert!(matches!(
                Changeset::from_file(&config, &path),
                Err(ResolveError::InvalidChangeset { .. })
            ));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_repeated_or_misplaced_separators() {
        let root = temp_dir("invalid-separators");
        let config = config_with_packages(&["api"]);
        for (name, content) in [
            ("missing", "api: patch\n\nSummary.\n"),
            ("missing-closing", "---\napi: patch\nSummary.\n"),
            (
                "legacy-with-trailing-marker",
                "api: patch\n---\n\nSummary.\n---\n",
            ),
            ("repeated", "---\napi: patch\n---\n\nSummary.\n---\n"),
        ] {
            let path = root.join(format!("{name}.md"));
            fs::write(&path, content).unwrap();

            assert!(matches!(
                Changeset::from_file(&config, &path),
                Err(ResolveError::InvalidChangeset { .. })
            ));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepts_legacy_changesets_without_an_opening_marker() {
        let root = temp_dir("legacy-without-opening-marker");
        let path = root.join("legacy.md");
        fs::write(&path, "api: patch\n---\n\nSummary.\n").unwrap();

        let changeset = Changeset::from_file(&config_with_packages(&["api"]), &path).unwrap();

        assert_eq!(changeset.summary, "Summary.");
        assert_eq!(changeset.packages.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_empty_packages_and_summary_when_loading() {
        let root = temp_dir("empty-fields");
        let config = config_with_packages(&["api"]);
        for (name, content) in [
            ("empty-packages", "---\n{}\n---\n\nSummary.\n"),
            ("empty-summary", "---\napi: patch\n---\n\n"),
        ] {
            let path = root.join(format!("{name}.md"));
            fs::write(&path, content).unwrap();

            assert!(matches!(
                Changeset::from_file(&config, &path),
                Err(ResolveError::InvalidChangeset { .. })
            ));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stores_the_configured_changeset_root() {
        let root = temp_dir("root-path");
        let changeset = Changeset::new("sample".to_string(), Path::new(&root));

        assert_eq!(changeset.root_path, root);

        fs::remove_dir_all(changeset.root_path).unwrap();
    }
}
