use std::path::Path;

use gix::{ObjectId, Repository};
use octocrab::Octocrab;

use semifold_resolver::error::ResolveError;

use crate::github::{GitHubFailure, GitHubOperation};
use crate::{RELEASE_MARKER_END, RELEASE_MARKER_PREFIX};

#[derive(Debug)]
pub struct CommitInfo {
    pub oid: ObjectId,
    pub author: Option<String>,
    pub message: String,
}

#[derive(Debug)]
pub struct PrInfo {
    pub number: u64,
    pub author: Option<String>,
    pub url: Option<String>,
}

pub async fn query_pr_for_commit(
    owner: &str,
    repo: &str,
    commit_info: &CommitInfo,
) -> Result<Option<PrInfo>, GitHubFailure> {
    let octocrab = if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        Octocrab::builder()
            .personal_token(token)
            .build()
            .map_err(|error| GitHubFailure::new(GitHubOperation::Initialize, error))?
    } else {
        Octocrab::builder()
            .build()
            .map_err(|error| GitHubFailure::new(GitHubOperation::Initialize, error))?
    };

    let prs = octocrab
        .repos(owner, repo)
        .list_pulls(commit_info.oid.to_string())
        .send()
        .await
        .map_err(|error| GitHubFailure::new(GitHubOperation::QueryCommitPullRequest, error))?;

    if let Some(pr) = prs.items.into_iter().next() {
        return Ok(Some(PrInfo {
            number: pr.number,
            author: pr.user.map(|u| u.login),
            url: pr.html_url.map(|u| u.to_string()),
        }));
    }

    if let Some(pr_number) = pull_request_number(&commit_info.message) {
        let pr = octocrab
            .pulls(owner, repo)
            .get(pr_number)
            .await
            .map_err(|error| GitHubFailure::new(GitHubOperation::QueryCommitPullRequest, error))?;
        return Ok(Some(PrInfo {
            number: pr.number,
            author: pr.user.map(|u| u.login),
            url: pr.html_url.map(|u| u.to_string()),
        }));
    }

    Ok(None)
}

fn pull_request_number(message: &str) -> Option<u64> {
    message.match_indices("(#").find_map(|(start, _)| {
        let remainder = message.get(start + 2..)?;
        let digits = remainder
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        if digits.is_empty()
            || !remainder
                .get(digits.len()..)
                .is_some_and(|suffix| suffix.starts_with(')'))
        {
            return None;
        }
        digits.parse().ok()
    })
}

pub fn find_first_commit_for_path(
    repo: &Repository,
    path: &Path,
) -> Result<Option<CommitInfo>, ResolveError> {
    find_first_commit(repo, path).map_err(|error| ResolveError::GitError {
        message: format!("{error:#}"),
    })
}

fn find_first_commit(repo: &Repository, path: &Path) -> gix::Result<Option<CommitInfo>> {
    let head = repo.head()?;
    let Some(id) = head.id() else {
        return Ok(None);
    };
    let commits = repo
        .rev_walk([id.detach()])
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()?
        .collect::<gix::Result<Vec<_>>>()?;
    let shallow = repo.shallow_commits()?;
    for info in commits.into_iter().rev() {
        let commit = repo.find_commit(info.id)?;
        let tree = commit.tree()?;
        let entry = tree.lookup_entry_by_path(path)?;
        let parent_id = if shallow.as_ref().is_some_and(|ids| ids.contains(&commit.id)) {
            None
        } else {
            commit.parent_ids().next()
        };
        let parent_entry = match parent_id {
            Some(parent) => repo
                .find_commit(parent)?
                .tree()?
                .lookup_entry_by_path(path)?,
            None => None,
        };
        let entry_key = entry
            .as_ref()
            .map(|entry| (entry.object_id(), entry.mode()));
        let parent_key = parent_entry
            .as_ref()
            .map(|entry| (entry.object_id(), entry.mode()));
        if entry_key != parent_key {
            let author = commit.author()?;
            let message = commit.message_raw()?;
            return Ok(Some(CommitInfo {
                oid: commit.id,
                author: std::str::from_utf8(author.name).ok().map(ToOwned::to_owned),
                message: String::from_utf8_lossy(message).into_owned(),
            }));
        }
    }
    Ok(None)
}

/// Renders the complete changelog content after inserting one newest entry.
pub fn render_changelog<P: AsRef<Path>>(
    path: P,
    content: Option<&str>,
    new_entry: &str,
    version: &str,
    require_marker: bool,
) -> Result<String, ResolveError> {
    let path = path.as_ref();
    let header = "# Changelog";

    let content = content
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{header}\n\n"));
    if content.lines().next() != Some(header) {
        return Err(ResolveError::InvalidChangelog {
            path: path.to_path_buf(),
            reason: "Invalid changelog: missing `# Changelog` root header".to_string(),
        });
    }

    let header_positions = content
        .match_indices(header)
        .filter(|(index, _)| {
            let before_is_boundary = *index == 0
                || content
                    .as_bytes()
                    .get(index.saturating_sub(1))
                    .is_some_and(|byte| *byte == b'\n');
            let after_index = index + header.len();
            let after_is_boundary = content
                .as_bytes()
                .get(after_index)
                .is_none_or(|byte| *byte == b'\n' || *byte == b'\r');
            before_is_boundary && after_is_boundary
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let Some(insert_pos) = header_positions.first().copied() else {
        return Err(ResolveError::InvalidChangelog {
            path: path.to_path_buf(),
            reason: "No `# Changelog` header found in file".to_string(),
        });
    };
    if header_positions.len() != 1 {
        return Err(ResolveError::InvalidChangelog {
            path: path.to_path_buf(),
            reason: "Multiple `# Changelog` headers found in file".to_string(),
        });
    }
    let has_markers = validate_release_markers(path, &content)?;
    let use_marker = require_marker || has_markers;

    let after_header_pos = insert_pos + header.len();
    let before = &content[..after_header_pos].trim_end_matches('\n');
    let after = &content[after_header_pos..].trim_start_matches('\n');
    let new_entry = if use_marker {
        let version =
            semver::Version::parse(version).map_err(|error| ResolveError::InvalidChangelog {
                path: path.to_path_buf(),
                reason: format!("Invalid release marker version {version}: {error}"),
            })?;
        format!(
            "{RELEASE_MARKER_PREFIX}{version} -->\n{}\n{RELEASE_MARKER_END}",
            new_entry.trim_matches('\n')
        )
    } else {
        new_entry.trim_start().to_string()
    };

    let mut new_content = String::with_capacity(content.len() + new_entry.len() + 4);
    new_content.push_str(before);
    new_content.push_str("\n\n");
    new_content.push_str(&new_entry);
    if !after.is_empty() {
        if new_entry.ends_with('\n') {
            new_content.push('\n');
        } else {
            new_content.push_str("\n\n");
        }
        new_content.push_str(after.trim_end_matches('\n'));
    }
    new_content.push('\n');

    Ok(new_content)
}

pub(crate) fn validate_release_markers(path: &Path, content: &str) -> Result<bool, ResolveError> {
    let mut open = false;
    let mut found = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(version) = release_marker_version(trimmed) {
            if open {
                return Err(invalid_marker(path, "Nested release marker"));
            }
            semver::Version::parse(version).map_err(|error| {
                invalid_marker(
                    path,
                    &format!("Invalid release marker version {version}: {error}"),
                )
            })?;
            open = true;
            found = true;
        } else if trimmed == RELEASE_MARKER_END {
            if !open {
                return Err(invalid_marker(
                    path,
                    "Release end marker has no matching start",
                ));
            }
            open = false;
        } else if trimmed.starts_with("<!-- semifold:release") {
            return Err(invalid_marker(path, "Malformed release marker"));
        }
    }
    if open {
        return Err(invalid_marker(
            path,
            "Release start marker has no matching end",
        ));
    }
    Ok(found)
}

pub(crate) fn release_marker_version(line: &str) -> Option<&str> {
    line.strip_prefix(RELEASE_MARKER_PREFIX)
        .and_then(|value| value.strip_suffix(" -->"))
}

fn invalid_marker(path: &Path, reason: &str) -> ResolveError {
    ResolveError::InvalidChangelog {
        path: path.to_path_buf(),
        reason: reason.to_string(),
    }
}

pub async fn insert_changelog<P: AsRef<Path>>(
    path: P,
    new_entry: &str,
    version: &str,
    require_marker: bool,
) -> Result<(), ResolveError> {
    let path = path.as_ref();
    let content = if path.exists() {
        Some(std::fs::read_to_string(path)?)
    } else {
        None
    };
    let new_content =
        render_changelog(path, content.as_deref(), new_entry, version, require_marker)?;
    std::fs::write(path, new_content)?;
    Ok(())
}

#[cfg(test)]
mod git_history_tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn history_finds_the_original_change_and_preserves_author_and_message() {
        let root = std::env::temp_dir().join(format!(
            "semifold-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = gix::init(&root).unwrap();
        assert!(
            find_first_commit_for_path(&repo, Path::new("change.md"))
                .unwrap()
                .is_none()
        );
        let mut time = gix::date::parse::TimeBuf::default();
        let mut identity = gix::actor::Signature {
            name: "作者".into(),
            email: "author@example.invalid".into(),
            time: gix::date::Time::new(1_700_000_000, 0),
        };
        let empty = repo.empty_tree().id;
        let initial = repo
            .commit_as(
                identity.to_ref(&mut time),
                identity.to_ref(&mut gix::date::parse::TimeBuf::default()),
                "HEAD",
                "initial",
                empty,
                std::iter::empty::<gix::ObjectId>(),
            )
            .unwrap()
            .detach();
        let blob = repo.write_blob(b"first\n").unwrap().detach();
        let mut tree = repo.edit_tree(empty).unwrap();
        tree.upsert("change.md", gix::objs::tree::EntryKind::Blob, blob)
            .unwrap();
        let tree_id = tree.write().unwrap().detach();
        identity.time.seconds += 1;
        let first = repo
            .commit_as(
                identity.to_ref(&mut time),
                identity.to_ref(&mut gix::date::parse::TimeBuf::default()),
                "HEAD",
                "original (#42)",
                tree_id,
                [initial],
            )
            .unwrap()
            .detach();
        identity.time.seconds += 1;
        repo.commit_as(
            identity.to_ref(&mut time),
            identity.to_ref(&mut gix::date::parse::TimeBuf::default()),
            "HEAD",
            "later",
            tree_id,
            [first],
        )
        .unwrap();
        let info = find_first_commit_for_path(&repo, Path::new("change.md"))
            .unwrap()
            .unwrap();
        assert_eq!(info.oid, first);
        assert_eq!(info.author.as_deref(), Some("作者"));
        assert_eq!(info.message, "original (#42)");
        assert!(
            find_first_commit_for_path(&repo, Path::new("missing"))
                .unwrap()
                .is_none()
        );
        let newest = repo.head_commit().unwrap().id;
        fs::write(repo.git_dir().join("shallow"), format!("{newest}\n")).unwrap();
        drop(repo);
        let shallow = gix::open(&root).unwrap();
        assert_eq!(
            find_first_commit_for_path(&shallow, Path::new("change.md"))
                .unwrap()
                .unwrap()
                .oid,
            newest
        );
        drop(shallow);
        fs::remove_dir_all(root).unwrap();
    }
}
