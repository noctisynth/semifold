use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::future::Future;
use std::io::Read;
use std::pin::Pin;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use glob::{MatchOptions, Pattern};

pub(crate) const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_OPERATION_FILE_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_OPERATION_PATHS: usize = 10_000;

/// Future returned by a runtime-neutral plugin file backend.
pub type PluginFileFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, PluginFileError>> + Send + 'a>>;

/// Host-controlled file capability exposed to a plugin operation.
pub trait PluginFileClient: fmt::Debug + Send + Sync + 'static {
    fn list_files(&self, pattern: &str) -> PluginFileFuture<'_, Vec<String>>;

    fn read_text(&self, path: &str) -> PluginFileFuture<'_, String>;
}

/// Default file capability. Plugins cannot inspect the project unless the host injects a client.
#[derive(Clone, Copy, Debug, Default)]
pub struct DenyPluginFileClient;

impl PluginFileClient for DenyPluginFileClient {
    fn list_files(&self, _pattern: &str) -> PluginFileFuture<'_, Vec<String>> {
        Box::pin(async { Err(PluginFileError::NotConfigured) })
    }

    fn read_text(&self, _path: &str) -> PluginFileFuture<'_, String> {
        Box::pin(async { Err(PluginFileError::NotConfigured) })
    }
}

/// Project-scoped implementation of the plugin file capability.
///
/// The declared patterns are an allowlist: `list_files` only accepts an exact pattern from that
/// set, while `read_text` only accepts a normalized relative path matched by at least one pattern.
#[derive(Clone, Debug)]
pub struct ScopedPluginFileClient {
    root: Utf8PathBuf,
    patterns: Arc<BTreeMap<String, AllowedPattern>>,
}

#[derive(Clone, Debug)]
struct AllowedPattern {
    compiled: Pattern,
    segments: Vec<AllowedPatternSegment>,
}

#[derive(Clone, Debug)]
enum AllowedPatternSegment {
    Recursive,
    Pattern(Pattern),
}

impl AllowedPattern {
    fn new(pattern: &str) -> Result<Self, PluginFileError> {
        let compiled = Pattern::new(pattern).map_err(|source| PluginFileError::InvalidPattern {
            pattern: pattern.to_owned(),
            reason: source.to_string(),
        })?;
        let segments = pattern
            .split('/')
            .map(|segment| {
                if segment == "**" {
                    Ok(AllowedPatternSegment::Recursive)
                } else {
                    Pattern::new(segment)
                        .map(AllowedPatternSegment::Pattern)
                        .map_err(|source| PluginFileError::InvalidPattern {
                            pattern: pattern.to_owned(),
                            reason: source.to_string(),
                        })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { compiled, segments })
    }

    fn matches(&self, path: &str) -> bool {
        self.compiled.matches_with(path, match_options())
    }

    fn can_match_below(&self, directory: &Utf8Path) -> bool {
        let mut states = BTreeSet::new();
        self.add_recursive_closure(0, &mut states);
        for component in directory
            .as_str()
            .split('/')
            .filter(|part| !part.is_empty())
        {
            let mut next = BTreeSet::new();
            for state in states {
                match self.segments.get(state) {
                    Some(AllowedPatternSegment::Recursive) if !component.starts_with('.') => {
                        self.add_recursive_closure(state, &mut next);
                    }
                    Some(AllowedPatternSegment::Pattern(pattern))
                        if pattern.matches_with(component, match_options()) =>
                    {
                        self.add_recursive_closure(state + 1, &mut next);
                    }
                    _ => {}
                }
            }
            states = next;
            if states.is_empty() {
                return false;
            }
        }
        states.into_iter().any(|state| state < self.segments.len())
    }

    fn add_recursive_closure(&self, mut state: usize, states: &mut BTreeSet<usize>) {
        states.insert(state);
        while matches!(
            self.segments.get(state),
            Some(AllowedPatternSegment::Recursive)
        ) {
            state += 1;
            states.insert(state);
        }
    }
}

impl ScopedPluginFileClient {
    pub fn new(
        root: impl Into<Utf8PathBuf>,
        read_patterns: impl IntoIterator<Item = String>,
    ) -> Result<Self, PluginFileError> {
        let configured_root = root.into();
        let canonical_root = std::fs::canonicalize(&configured_root).map_err(|source| {
            PluginFileError::ResolveRoot {
                root: configured_root.clone(),
                source,
            }
        })?;
        let root = Utf8PathBuf::from_path_buf(canonical_root).map_err(|path| {
            PluginFileError::NonUtf8Root {
                root: path.display().to_string(),
            }
        })?;
        if !root.is_dir() {
            return Err(PluginFileError::RootNotDirectory { root });
        }

        let mut patterns = BTreeMap::new();
        for pattern in read_patterns {
            validate_protocol_path(&pattern, PathKind::Pattern)?;
            let compiled = AllowedPattern::new(&pattern)?;
            patterns.insert(pattern, compiled);
        }

        Ok(Self {
            root,
            patterns: Arc::new(patterns),
        })
    }

    fn list_files_sync(&self, requested_pattern: &str) -> Result<Vec<String>, PluginFileError> {
        validate_protocol_path(requested_pattern, PathKind::Pattern)?;
        let pattern = self.patterns.get(requested_pattern).ok_or_else(|| {
            PluginFileError::PatternNotAllowed {
                pattern: requested_pattern.to_owned(),
            }
        })?;
        let mut matches = BTreeSet::new();
        self.walk_directory(
            &self.root,
            Utf8Path::new(""),
            pattern,
            &mut BTreeSet::new(),
            &mut matches,
        )?;
        Ok(matches.into_iter().collect())
    }

    fn walk_directory(
        &self,
        directory: &Utf8Path,
        logical_directory: &Utf8Path,
        pattern: &AllowedPattern,
        ancestors: &mut BTreeSet<Utf8PathBuf>,
        matches: &mut BTreeSet<String>,
    ) -> Result<(), PluginFileError> {
        let canonical = self.resolve_inside_root(directory, logical_directory.as_str())?;
        if !ancestors.insert(canonical.clone()) {
            return Err(PluginFileError::DirectoryCycle {
                path: logical_directory.to_owned(),
            });
        }
        let entries =
            std::fs::read_dir(&canonical).map_err(|source| PluginFileError::ReadDirectory {
                path: canonical.clone(),
                source,
            })?;
        for entry in entries {
            let entry = entry.map_err(|source| PluginFileError::ReadDirectory {
                path: canonical.clone(),
                source,
            })?;
            let path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
                PluginFileError::NonUtf8Path {
                    path: path.display().to_string(),
                }
            })?;
            let name =
                entry
                    .file_name()
                    .into_string()
                    .map_err(|name| PluginFileError::NonUtf8Path {
                        path: name.to_string_lossy().into_owned(),
                    })?;
            // Protocol paths always use '/', independently of the host OS.
            let logical_path = if logical_directory.as_str().is_empty() {
                Utf8PathBuf::from(name)
            } else {
                Utf8PathBuf::from(format!("{logical_directory}/{name}"))
            };
            let file_type = entry
                .file_type()
                .map_err(|source| PluginFileError::InspectPath {
                    path: path.clone(),
                    source,
                })?;

            if file_type.is_dir() {
                if pattern.can_match_below(&logical_path) {
                    self.walk_directory(&path, &logical_path, pattern, ancestors, matches)?;
                }
                continue;
            }

            if file_type.is_symlink() {
                self.collect_symlink(&path, &logical_path, pattern, ancestors, matches)?;
                continue;
            }

            if file_type.is_file() && pattern.matches(logical_path.as_str()) {
                insert_match(matches, logical_path.as_str())?;
            }
        }
        ancestors.remove(&canonical);
        Ok(())
    }

    fn collect_symlink(
        &self,
        path: &Utf8Path,
        logical_path: &Utf8Path,
        pattern: &AllowedPattern,
        ancestors: &mut BTreeSet<Utf8PathBuf>,
        matches: &mut BTreeSet<String>,
    ) -> Result<(), PluginFileError> {
        let matches_file = pattern.matches(logical_path.as_str());
        let matches_below = pattern.can_match_below(logical_path);
        if !matches_file && !matches_below {
            return Ok(());
        }
        let canonical = self.resolve_inside_root(path, logical_path.as_str())?;
        if canonical.is_dir() && matches_below {
            self.walk_directory(&canonical, logical_path, pattern, ancestors, matches)?;
        } else if canonical.is_file() && matches_file {
            insert_match(matches, logical_path.as_str())?;
        }
        Ok(())
    }

    fn read_text_sync(&self, requested_path: &str) -> Result<String, PluginFileError> {
        validate_protocol_path(requested_path, PathKind::File)?;
        if !self
            .patterns
            .values()
            .any(|pattern| pattern.matches(requested_path))
        {
            return Err(PluginFileError::PathNotAllowed {
                path: requested_path.to_owned(),
            });
        }

        let path = self.root.join(requested_path);
        let canonical = self.resolve_inside_root(&path, requested_path)?;
        let metadata = canonical
            .metadata()
            .map_err(|source| PluginFileError::InspectPath {
                path: canonical.clone(),
                source,
            })?;
        if !metadata.is_file() {
            return Err(PluginFileError::NotAFile {
                path: requested_path.to_owned(),
            });
        }
        let actual = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
        if actual > MAX_FILE_BYTES {
            return Err(PluginFileError::FileTooLarge {
                path: requested_path.to_owned(),
                actual,
                maximum: MAX_FILE_BYTES,
            });
        }

        let file = File::open(&canonical).map_err(|source| PluginFileError::ReadFile {
            path: requested_path.to_owned(),
            source,
        })?;
        let maximum =
            u64::try_from(MAX_FILE_BYTES).map_err(|source| PluginFileError::InternalLimit {
                reason: source.to_string(),
            })?;
        let mut bytes = Vec::with_capacity(actual);
        file.take(maximum.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| PluginFileError::ReadFile {
                path: requested_path.to_owned(),
                source,
            })?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(PluginFileError::FileTooLarge {
                path: requested_path.to_owned(),
                actual: bytes.len(),
                maximum: MAX_FILE_BYTES,
            });
        }
        String::from_utf8(bytes).map_err(|source| PluginFileError::InvalidUtf8 {
            path: requested_path.to_owned(),
            source,
        })
    }

    fn resolve_inside_root(
        &self,
        path: &Utf8Path,
        relative: &str,
    ) -> Result<Utf8PathBuf, PluginFileError> {
        let canonical =
            std::fs::canonicalize(path).map_err(|source| PluginFileError::ResolvePath {
                path: relative.to_owned(),
                source,
            })?;
        let canonical =
            Utf8PathBuf::from_path_buf(canonical).map_err(|path| PluginFileError::NonUtf8Path {
                path: path.display().to_string(),
            })?;
        if !canonical.starts_with(&self.root) {
            return Err(PluginFileError::OutsideProjectRoot {
                path: relative.to_owned(),
            });
        }
        Ok(canonical)
    }
}

impl PluginFileClient for ScopedPluginFileClient {
    fn list_files(&self, pattern: &str) -> PluginFileFuture<'_, Vec<String>> {
        let pattern = pattern.to_owned();
        Box::pin(async move { self.list_files_sync(&pattern) })
    }

    fn read_text(&self, path: &str) -> PluginFileFuture<'_, String> {
        let path = path.to_owned();
        Box::pin(async move { self.read_text_sync(&path) })
    }
}

fn insert_match(matches: &mut BTreeSet<String>, path: &str) -> Result<(), PluginFileError> {
    matches.insert(path.to_owned());
    if matches.len() > MAX_OPERATION_PATHS {
        return Err(PluginFileError::TooManyPaths {
            actual: matches.len(),
            maximum: MAX_OPERATION_PATHS,
        });
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PathKind {
    File,
    Pattern,
}

pub(crate) fn validate_file_path(value: &str) -> Result<(), PluginFileError> {
    validate_protocol_path(value, PathKind::File)
}

pub(crate) fn validate_pattern_path(value: &str) -> Result<(), PluginFileError> {
    validate_protocol_path(value, PathKind::Pattern)
}

pub(crate) fn matches_pattern(pattern: &str, path: &str) -> Result<bool, PluginFileError> {
    let pattern = Pattern::new(pattern).map_err(|source| PluginFileError::InvalidPattern {
        pattern: pattern.to_owned(),
        reason: source.to_string(),
    })?;
    Ok(pattern.matches_with(path, match_options()))
}

fn validate_protocol_path(value: &str, kind: PathKind) -> Result<(), PluginFileError> {
    let invalid = value.is_empty()
        || value.contains('\\')
        || value.starts_with('/')
        || value.ends_with('/')
        || value.split('/').any(|segment| {
            segment.is_empty()
                || segment == "."
                || segment == ".."
                || is_windows_drive_segment(segment)
        });
    if invalid {
        return match kind {
            PathKind::File => Err(PluginFileError::InvalidPath {
                path: value.to_owned(),
            }),
            PathKind::Pattern => Err(PluginFileError::InvalidPattern {
                pattern: value.to_owned(),
                reason: "patterns must be normalized project-relative UTF-8 paths".to_owned(),
            }),
        };
    }
    Ok(())
}

fn is_windows_drive_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

const fn match_options() -> MatchOptions {
    MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: true,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginFileError {
    #[error("plugin file access is not configured")]
    NotConfigured,
    #[error("failed to resolve plugin project root `{root}`: {source}")]
    ResolveRoot {
        root: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("plugin project root is not UTF-8: `{root}`")]
    NonUtf8Root { root: String },
    #[error("plugin project root is not a directory: `{root}`")]
    RootNotDirectory { root: Utf8PathBuf },
    #[error("invalid plugin read pattern `{pattern}`: {reason}")]
    InvalidPattern { pattern: String, reason: String },
    #[error("plugin read pattern is not allowed: `{pattern}`")]
    PatternNotAllowed { pattern: String },
    #[error("invalid plugin file path: `{path}`")]
    InvalidPath { path: String },
    #[error("plugin file path is not allowed: `{path}`")]
    PathNotAllowed { path: String },
    #[error("plugin file path is outside the project root: `{path}`")]
    OutsideProjectRoot { path: String },
    #[error("plugin file path is not UTF-8: `{path}`")]
    NonUtf8Path { path: String },
    #[error("failed to read plugin directory `{path}`: {source}")]
    ReadDirectory {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect plugin file path `{path}`: {source}")]
    InspectPath {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to resolve plugin file path `{path}`: {source}")]
    ResolvePath {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("plugin file traversal encountered a directory cycle at `{path}`")]
    DirectoryCycle { path: Utf8PathBuf },
    #[error("plugin file path is not a regular file: `{path}`")]
    NotAFile { path: String },
    #[error("plugin file `{path}` contains {actual} bytes; maximum is {maximum}")]
    FileTooLarge {
        path: String,
        actual: usize,
        maximum: usize,
    },
    #[error("failed to read plugin file `{path}`: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("plugin file `{path}` is not valid UTF-8: {source}")]
    InvalidUtf8 {
        path: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("plugin file listing returned {actual} paths; maximum is {maximum}")]
    TooManyPaths { actual: usize, maximum: usize },
    #[error("plugin file capability `{method}` requires a string argument")]
    InvalidArgument { method: &'static str },
    #[error("plugin file capability host is unavailable")]
    HostUnavailable,
    #[error("plugin file capability budget state is unavailable")]
    BudgetStateUnavailable,
    #[error("plugin file capability returned path `{path}` that does not match `{pattern}`")]
    ReturnedPathDoesNotMatch { path: String, pattern: String },
    #[error("plugin file reads returned {actual} bytes in this operation; maximum is {maximum}")]
    OperationBytesExceeded { actual: usize, maximum: usize },
    #[error("invalid internal plugin file limit: {reason}")]
    InternalLimit { reason: String },
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn fixture_root(test: &str) -> Utf8PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "semifold-plugin-file-{}-{test}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Utf8PathBuf::from_path_buf(root).unwrap()
    }

    #[test]
    fn lists_declared_patterns_in_sorted_order_and_reads_matching_text() {
        let root = fixture_root("list-and-read");
        fs::create_dir_all(root.join("packages/zeta")).unwrap();
        fs::create_dir_all(root.join("packages/alpha")).unwrap();
        fs::write(root.join("packages/zeta/package.json"), "zeta").unwrap();
        fs::write(root.join("packages/alpha/package.json"), "alpha").unwrap();
        fs::write(root.join("packages/alpha/ignored.toml"), "ignored").unwrap();
        let client =
            ScopedPluginFileClient::new(root.clone(), ["packages/**/package.json".to_owned()])
                .unwrap();

        assert_eq!(
            client.list_files_sync("packages/**/package.json").unwrap(),
            vec![
                "packages/alpha/package.json".to_owned(),
                "packages/zeta/package.json".to_owned()
            ]
        );
        assert_eq!(
            client
                .read_text_sync("packages/alpha/package.json")
                .unwrap(),
            "alpha"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_undeclared_patterns_and_non_matching_paths() {
        let root = fixture_root("authorization");
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::write(root.join("secret.txt"), "secret").unwrap();
        let client =
            ScopedPluginFileClient::new(root.clone(), ["package.json".to_owned()]).unwrap();

        assert!(matches!(
            client.list_files_sync("*.json"),
            Err(PluginFileError::PatternNotAllowed { .. })
        ));
        assert!(matches!(
            client.read_text_sync("secret.txt"),
            Err(PluginFileError::PathNotAllowed { .. })
        ));
        assert!(matches!(
            client.read_text_sync("../secret.txt"),
            Err(PluginFileError::InvalidPath { .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_utf8_file_content() {
        let root = fixture_root("invalid-utf8");
        fs::write(root.join("invalid.txt"), [0xff, 0xfe]).unwrap();
        let client = ScopedPluginFileClient::new(root.clone(), ["*.txt".to_owned()]).unwrap();

        assert!(matches!(
            client.read_text_sync("invalid.txt"),
            Err(PluginFileError::InvalidUtf8 { .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_files_larger_than_the_per_file_budget() {
        let root = fixture_root("file-budget");
        fs::write(root.join("large.txt"), vec![b'x'; MAX_FILE_BYTES + 1]).unwrap();
        let client = ScopedPluginFileClient::new(root.clone(), ["*.txt".to_owned()]).unwrap();

        assert!(matches!(
            client.read_text_sync("large.txt"),
            Err(PluginFileError::FileTooLarge {
                actual,
                maximum: MAX_FILE_BYTES,
                ..
            }) if actual == MAX_FILE_BYTES + 1
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_project_root() {
        use std::os::unix::fs::symlink;

        let root = fixture_root("symlink-root");
        let outside = fixture_root("symlink-outside");
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        symlink(outside.join("secret.txt"), root.join("secret.txt")).unwrap();
        let client = ScopedPluginFileClient::new(root.clone(), ["*.txt".to_owned()]).unwrap();

        assert!(matches!(
            client.list_files_sync("*.txt"),
            Err(PluginFileError::OutsideProjectRoot { .. })
        ));
        assert!(matches!(
            client.read_text_sync("secret.txt"),
            Err(PluginFileError::OutsideProjectRoot { .. })
        ));
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn traverses_in_root_directory_symlinks_and_rejects_external_targets() {
        use std::os::unix::fs::symlink;

        let root = fixture_root("directory-symlinks");
        let outside = fixture_root("external-directory");
        fs::create_dir_all(root.join("actual")).unwrap();
        fs::write(root.join("actual/package.json"), "{}").unwrap();
        fs::write(outside.join("package.json"), "{}").unwrap();
        symlink(root.join("actual"), root.join("linked")).unwrap();
        symlink(&outside, root.join("external")).unwrap();
        let client = ScopedPluginFileClient::new(
            root.clone(),
            [
                "linked/**/*.json".to_owned(),
                "external/**/*.json".to_owned(),
            ],
        )
        .unwrap();

        assert_eq!(
            client.list_files_sync("linked/**/*.json").unwrap(),
            vec!["linked/package.json".to_owned()]
        );
        assert!(matches!(
            client.list_files_sync("external/**/*.json"),
            Err(PluginFileError::OutsideProjectRoot { .. })
        ));
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_directory_symlink_cycles() {
        use std::os::unix::fs::symlink;

        let root = fixture_root("symlink-cycle");
        fs::create_dir_all(root.join("data")).unwrap();
        symlink(&root, root.join("data/back")).unwrap();
        let client =
            ScopedPluginFileClient::new(root.clone(), ["data/**/*.json".to_owned()]).unwrap();

        assert!(matches!(
            client.list_files_sync("data/**/*.json"),
            Err(PluginFileError::DirectoryCycle { .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
