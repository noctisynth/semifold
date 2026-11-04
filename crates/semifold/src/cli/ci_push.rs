//! The only production adapter allowed to use libgit2.

use std::path::Path;

use git2::{Cred, PushOptions, RemoteCallbacks, Repository};
use rust_i18n::t;

pub(super) struct PushAdapter {
    repository: Repository,
}

impl PushAdapter {
    /// Preflight before release files or refs change. No network access occurs.
    pub(super) fn open(root: &Path) -> anyhow::Result<Self> {
        let repository = Repository::open(root)
            .map_err(|error| anyhow::anyhow!(t!("cli.ci.push_preflight_failed", error = error)))?;
        repository
            .find_remote("origin")
            .map_err(|error| anyhow::anyhow!(t!("cli.ci.push_preflight_failed", error = error)))?;
        Ok(Self { repository })
    }

    pub(super) fn force_push(&self, token: &str, branch: &str) -> anyhow::Result<()> {
        let mut callbacks = RemoteCallbacks::new();
        let token = token.to_owned();
        callbacks.credentials(move |_url, username, _allowed| {
            if username.is_some() {
                Cred::userpass_plaintext(&token, "")
            } else {
                Cred::userpass_plaintext("x-access-token", &token)
            }
        });
        let mut options = PushOptions::new();
        options.remote_callbacks(callbacks);
        let mut remote = self.repository.find_remote("origin")?;
        remote.push(
            &[&format!("+refs/heads/{branch}:refs/heads/{branch}")],
            Some(&mut options),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn libgit2_supports_https_transport() {
        assert!(git2::Version::get().https());
    }

    #[test]
    fn preflight_handle_pushes_commits_created_later_by_gix() {
        let sandbox = std::env::temp_dir().join(format!(
            "semifold-push-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&sandbox).unwrap();
        let root = sandbox.join("work");
        let remote_root = sandbox.join("remote.git");
        let repository = gix::init(&root).unwrap();
        gix::init_bare(&remote_root).unwrap();
        let identity = gix::actor::Signature {
            name: "Fixture".into(),
            email: "fixture@example.invalid".into(),
            time: gix::date::Time::now_local_or_utc(),
        };
        let initial = repository
            .commit_as(
                identity.to_ref(&mut gix::date::parse::TimeBuf::default()),
                identity.to_ref(&mut gix::date::parse::TimeBuf::default()),
                "HEAD",
                "initial",
                repository.empty_tree().id,
                std::iter::empty::<gix::ObjectId>(),
            )
            .unwrap()
            .detach();
        Repository::open(&root)
            .unwrap()
            .remote("origin", remote_root.to_str().unwrap())
            .unwrap();
        let push = PushAdapter::open(&root).unwrap();
        fs::write(root.join("release-file"), "version\n").unwrap();
        let id = semifold_engine::git::commit_release(&repository, "release", "release").unwrap();
        assert_ne!(initial, id);
        push.force_push("unused-local-token", "release").unwrap();
        assert_eq!(
            gix::open(&remote_root)
                .unwrap()
                .find_reference("refs/heads/release")
                .unwrap()
                .id()
                .detach(),
            id
        );
        drop(push);
        drop(repository);
        fs::remove_dir_all(sandbox).unwrap();
    }
}
