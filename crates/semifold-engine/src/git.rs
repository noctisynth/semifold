//! Local Git operations. Remote transport belongs to the CLI push adapter.

use std::{collections::BTreeMap, fs, path::Path};

use gix::{
    bstr::{BString, ByteSlice},
    index::entry::Mode,
};
use thiserror::Error;

/// Open a known repository without relaxing ownership validation.
pub fn open_repository(path: &Path) -> gix::Result<gix::Repository> {
    gix::open_opts(path, open_options())
}

/// Discover a worktree using the same trust policy at both trust levels.
pub fn discover_repository(path: &Path) -> gix::Result<gix::Repository> {
    gix::discover_opts(path, Default::default(), open_options())
}

fn open_options() -> gix::open::Options {
    gix::open::Options::default().bail_if_untrusted(true)
}

/// Include staged, unstaged and untracked changes, independently of status UI config.
pub fn has_changes(repository: &gix::Repository) -> gix::Result<bool> {
    let walk = repository.dirwalk_options()?;
    let mut changes = repository
        .status(gix::progress::Discard)?
        .index_worktree_options_mut(|options| options.dirwalk_options = Some(walk))
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(Vec::new())?;
    match changes.next() {
        Some(change) => change.map(|_| true),
        None => Ok(false),
    }
}

/// Git errors retain their nested context when crossing the CLI boundary.
pub fn diagnostic(error: &gix::Error) -> String {
    format!("{error:#}")
}

#[derive(Debug, Error)]
pub enum GitWriteError {
    #[error("{0:#}")]
    Git(#[from] gix::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("a non-bare repository with an existing HEAD is required")]
    MissingWorktree,
    #[error("cannot create a release commit from a conflicted or sparse index")]
    UnsupportedIndex,
    #[error("unsupported working tree entry: {0}")]
    UnsupportedEntry(BString),
}

/// Stage the worktree and create a release commit without checking out another tree.
/// The new branch starts at the current HEAD, so checkout would only rewrite the
/// same tree over the version files which the release operation just generated.
pub fn commit_release(
    repository: &gix::Repository,
    branch: &str,
    message: &str,
) -> Result<gix::ObjectId, GitWriteError> {
    use gix::refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit},
    };

    let head = repository.find_reference("HEAD")?;
    let old_head: Target = head.target().into();
    let parent = repository.head_commit()?.id;
    let branch_name = format!("refs/heads/{branch}");
    let branch_name =
        gix::refs::FullName::try_from(branch_name.as_str()).map_err(gix::Error::from_error)?;
    let previous_branch = match repository.try_find_reference(branch_name.as_ref())? {
        Some(reference) => PreviousValue::MustExistAndMatch(reference.target().into()),
        None => PreviousValue::MustNotExist,
    };
    let tree = stage_worktree(repository)?;
    let identity = gix::actor::Signature {
        name: "github-actions[bot]".into(),
        email: "github-actions[bot]@users.noreply.github.com".into(),
        time: gix::date::Time::now_local_or_utc(),
    };
    let mut time_buf = gix::date::parse::TimeBuf::default();
    let signature = identity.to_ref(&mut time_buf);
    let commit = repository
        .new_commit_as(signature, signature, message, tree, [parent])?
        .id;
    let mut edits = vec![
        RefEdit {
            name: branch_name.clone(),
            deref: false,
            change: Change::Update {
                log: LogChange::default(),
                expected: previous_branch,
                new: Target::Object(commit),
            },
        },
        RefEdit {
            name: gix::refs::FullName::try_from("HEAD").map_err(gix::Error::from_error)?,
            deref: false,
            change: Change::Update {
                log: LogChange::default(),
                expected: PreviousValue::MustExistAndMatch(old_head.clone()),
                new: Target::Symbolic(branch_name.clone()),
            },
        },
    ];
    // Lock and verify the base ref as well: HEAD may still be symbolic while its
    // target has moved concurrently. Do not reset or advance that base branch.
    if let Target::Symbolic(base) = old_head
        && base != branch_name
    {
        edits.push(RefEdit {
            name: base,
            deref: false,
            change: Change::Update {
                log: LogChange::default(),
                expected: PreviousValue::MustExistAndMatch(Target::Object(parent)),
                new: Target::Object(parent),
            },
        });
    }
    repository.edit_references_as(edits, Some(signature))?;
    Ok(commit)
}

fn stage_worktree(repository: &gix::Repository) -> Result<gix::ObjectId, GitWriteError> {
    let root = repository.workdir().ok_or(GitWriteError::MissingWorktree)?;
    let original = repository.index_or_empty()?;
    if original.is_sparse()
        || original
            .entries()
            .iter()
            .any(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
    {
        return Err(GitWriteError::UnsupportedIndex);
    }
    let mut entries: BTreeMap<BString, (Mode, gix::ObjectId)> = original
        .entries()
        .iter()
        .map(|entry| (entry.path(&original).to_owned(), (entry.mode, entry.id)))
        .collect();
    let walk = repository.dirwalk_options()?;
    let changes = repository
        .status(gix::progress::Discard)?
        .index_worktree_options_mut(|options| options.dirwalk_options = Some(walk))
        .untracked_files(gix::status::UntrackedFiles::Files)
        .index_worktree_rewrites(None)
        .into_index_worktree_iter(Vec::new())?;
    let (mut filters, _) = repository.filter_pipeline(None)?;
    let capabilities = repository.filesystem_options()?;
    for change in changes {
        let change = change?;
        if change.summary().is_none() {
            continue;
        }
        let path = change.rela_path().to_owned();
        let relative = gix::path::from_bstr(path.as_bstr())?;
        let absolute = root.join(&relative);
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                entries.remove(&path);
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let previous = entries.get(&path).map(|(mode, _)| *mode);
        let (mode, id) = if metadata.file_type().is_symlink() {
            let target = fs::read_link(&absolute)?;
            let target = gix::path::into_bstr(target).map_err(gix::Error::from_error)?;
            (
                Mode::SYMLINK,
                repository.write_blob(target.as_bytes())?.detach(),
            )
        } else if metadata.is_dir() {
            let submodule = open_repository(&absolute)?;
            if submodule.workdir() != Some(absolute.as_path()) {
                return Err(GitWriteError::UnsupportedEntry(path));
            }
            (Mode::COMMIT, submodule.head_commit()?.id)
        } else if metadata.is_file() {
            let mut mode = if !capabilities.executable_bit {
                previous
                    .filter(|mode| *mode == Mode::FILE_EXECUTABLE)
                    .unwrap_or(Mode::FILE)
            } else {
                Mode::FILE
            };
            #[cfg(unix)]
            if capabilities.executable_bit {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o111 != 0 {
                    mode = Mode::FILE_EXECUTABLE;
                }
            }
            // Git for Windows represents symlinks as ordinary files when
            // core.symlinks=false; their contents are already the target bytes.
            let id = if !capabilities.symlink && previous == Some(Mode::SYMLINK) {
                mode = Mode::SYMLINK;
                repository.write_blob(fs::read(&absolute)?)?.detach()
            } else {
                let input = fs::File::open(&absolute)?;
                let mut converted = filters.convert_to_git(input, &relative, &original)?;
                repository.write_blob_stream(&mut converted)?.detach()
            };
            (mode, id)
        } else {
            return Err(GitWriteError::UnsupportedEntry(path));
        };
        entries.insert(path, (mode, id));
    }

    let mut tree = repository.edit_tree(gix::ObjectId::empty_tree(repository.object_hash()))?;
    for (path, (mode, id)) in &entries {
        let tree_mode = mode
            .to_tree_entry_mode()
            .ok_or(GitWriteError::UnsupportedIndex)?;
        tree.upsert(path, tree_mode.kind(), *id)?;
    }
    let tree = tree.write()?.detach();
    let mut index = repository.index_from_tree(&tree)?;
    index.write(Default::default())?;
    Ok(tree)
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
