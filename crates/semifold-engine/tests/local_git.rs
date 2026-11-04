#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use semifold_engine::git::{commit_release, discover_repository, has_changes, open_repository};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "semifold-local-git-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        git(&path, &["init", "-b", "main"]);
        git(&path, &["config", "user.name", "Fixture"]);
        git(&path, &["config", "user.email", "fixture@example.invalid"]);
        git(&path, &["config", "core.autocrlf", "false"]);
        Self(path)
    }

    fn initial_commit(&self) {
        git(&self.0, &["add", "."]);
        git(&self.0, &["commit", "-m", "initial"]);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", root.join(".git-test-global"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn stages_add_modify_delete_and_attributes_without_resetting_the_base_or_config() {
    let fixture = Fixture::new();
    let root = &fixture.0;
    fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
    fs::write(root.join(".gitattributes"), "*.txt text eol=lf\n").unwrap();
    fs::write(root.join("keep.txt"), "original\n").unwrap();
    fs::write(root.join("delete.txt"), "delete\n").unwrap();
    fixture.initial_commit();
    let base = git(root, &["rev-parse", "HEAD"]);
    let config = fs::read(root.join(".git/config")).unwrap();
    fs::write(root.join("keep.txt"), "staged\n").unwrap();
    git(root, &["add", "keep.txt"]);
    fs::write(root.join("keep.txt"), "latest\r\n").unwrap();
    fs::remove_file(root.join("delete.txt")).unwrap();
    fs::create_dir_all(root.join("new directory")).unwrap();
    fs::write(root.join("new directory/中文.txt"), "new\r\n").unwrap();
    fs::create_dir_all(root.join("ignored")).unwrap();
    fs::write(root.join("ignored/output"), "ignored").unwrap();
    let repository = open_repository(root).unwrap();
    assert!(has_changes(&repository).unwrap());
    let id = commit_release(&repository, "release", "release fixture").unwrap();
    drop(repository);
    assert_eq!(git(root, &["rev-parse", "main"]), base);
    assert_eq!(git(root, &["rev-parse", "HEAD^"]), base);
    assert_eq!(git(root, &["rev-parse", "HEAD"]), id.to_string());
    assert_eq!(git(root, &["symbolic-ref", "HEAD"]), "refs/heads/release");
    assert_eq!(git(root, &["show", "HEAD:keep.txt"]), "latest");
    assert_eq!(git(root, &["show", "HEAD:new directory/中文.txt"]), "new");
    assert_eq!(git(root, &["status", "--porcelain"]), "");
    assert_eq!(fs::read(root.join(".git/config")).unwrap(), config);
    assert_eq!(
        git(root, &["show", "-s", "--format=%an <%ae>"]),
        "github-actions[bot] <github-actions[bot]@users.noreply.github.com>"
    );
    assert!(!has_changes(&open_repository(root).unwrap()).unwrap());
}

#[test]
fn dirty_check_includes_untracked_and_staged_even_when_status_config_hides_them() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("tracked"), "base").unwrap();
    fixture.initial_commit();
    git(&fixture.0, &["config", "status.showUntrackedFiles", "no"]);
    fs::write(fixture.0.join("untracked"), "new").unwrap();
    assert!(has_changes(&open_repository(&fixture.0).unwrap()).unwrap());
    git(&fixture.0, &["add", "untracked"]);
    assert!(has_changes(&open_repository(&fixture.0).unwrap()).unwrap());
}

#[test]
fn discovery_and_commit_support_linked_worktrees_and_detached_head() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("tracked"), "base").unwrap();
    fixture.initial_commit();
    let linked = fixture.0.join("linked");
    git(
        &fixture.0,
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            "HEAD",
        ],
    );
    fs::create_dir_all(linked.join("nested")).unwrap();
    let repository = discover_repository(&linked.join("nested")).unwrap();
    assert_eq!(
        repository.workdir().unwrap().canonicalize().unwrap(),
        linked.canonicalize().unwrap()
    );
    fs::write(linked.join("tracked"), "release").unwrap();
    commit_release(&repository, "release-worktree", "linked release").unwrap();
    assert_eq!(
        git(&linked, &["symbolic-ref", "HEAD"]),
        "refs/heads/release-worktree"
    );
    assert_eq!(git(&linked, &["status", "--porcelain"]), "");
    assert_eq!(git(&fixture.0, &["show", "main:tracked"]), "base");
}

#[test]
fn conflicts_fail_without_changing_refs_or_index() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("tracked"), "base\n").unwrap();
    fixture.initial_commit();
    git(&fixture.0, &["checkout", "-b", "other"]);
    fs::write(fixture.0.join("tracked"), "other\n").unwrap();
    git(&fixture.0, &["commit", "-am", "other"]);
    git(&fixture.0, &["checkout", "main"]);
    fs::write(fixture.0.join("tracked"), "main\n").unwrap();
    git(&fixture.0, &["commit", "-am", "main"]);
    let output = Command::new("git")
        .args(["merge", "other"])
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let index = fs::read(fixture.0.join(".git/index")).unwrap();
    let head = git(&fixture.0, &["rev-parse", "HEAD"]);
    assert!(commit_release(&open_repository(&fixture.0).unwrap(), "release", "conflict").is_err());
    assert_eq!(fs::read(fixture.0.join(".git/index")).unwrap(), index);
    assert_eq!(git(&fixture.0, &["rev-parse", "HEAD"]), head);
}

#[test]
fn release_staging_updates_submodule_gitlinks_and_existing_release_branches() {
    let source = Fixture::new();
    fs::write(source.0.join("file"), "first").unwrap();
    source.initial_commit();
    let fixture = Fixture::new();
    fs::write(fixture.0.join("base"), "base").unwrap();
    git(
        &fixture.0,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            source.0.to_str().unwrap(),
            "module",
        ],
    );
    fixture.initial_commit();
    let base = git(&fixture.0, &["rev-parse", "HEAD"]);
    let module = fixture.0.join("module");
    git(&module, &["config", "user.name", "Fixture"]);
    git(
        &module,
        &["config", "user.email", "fixture@example.invalid"],
    );
    fs::write(module.join("file"), "second").unwrap();
    git(&module, &["commit", "-am", "second"]);
    let module_commit = git(&module, &["rev-parse", "HEAD"]);
    let first = commit_release(
        &open_repository(&fixture.0).unwrap(),
        "release",
        "submodule",
    )
    .unwrap();
    assert_eq!(
        git(&fixture.0, &["rev-parse", "HEAD:module"]),
        module_commit
    );
    fs::write(fixture.0.join("base"), "updated").unwrap();
    commit_release(
        &open_repository(&fixture.0).unwrap(),
        "release",
        "second release",
    )
    .unwrap();
    assert_eq!(git(&fixture.0, &["rev-parse", "HEAD^"]), first.to_string());
    assert_eq!(git(&fixture.0, &["rev-parse", "main"]), base);
    assert_eq!(git(&fixture.0, &["status", "--porcelain"]), "");
}

#[test]
fn required_filter_failure_does_not_replace_the_index_or_refs() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("base"), "base").unwrap();
    fixture.initial_commit();
    git(
        &fixture.0,
        &[
            "config",
            "filter.unavailable.clean",
            "semifold-fixture-missing-command",
        ],
    );
    git(
        &fixture.0,
        &["config", "filter.unavailable.required", "true"],
    );
    fs::write(
        fixture.0.join(".gitattributes"),
        "*.filtered filter=unavailable\n",
    )
    .unwrap();
    fs::write(fixture.0.join("file.filtered"), "content").unwrap();
    let index = fs::read(fixture.0.join(".git/index")).unwrap();
    let head = git(&fixture.0, &["rev-parse", "HEAD"]);
    assert!(
        commit_release(
            &open_repository(&fixture.0).unwrap(),
            "release",
            "filter failure"
        )
        .is_err()
    );
    assert_eq!(fs::read(fixture.0.join(".git/index")).unwrap(), index);
    assert_eq!(git(&fixture.0, &["rev-parse", "HEAD"]), head);
}

#[cfg(unix)]
#[test]
fn staging_preserves_executable_modes_and_symlink_targets() {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};
    let fixture = Fixture::new();
    fs::write(fixture.0.join("base"), "base").unwrap();
    fixture.initial_commit();
    fs::write(fixture.0.join("script"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(fixture.0.join("script"), fs::Permissions::from_mode(0o755)).unwrap();
    symlink("base", fixture.0.join("link")).unwrap();
    commit_release(&open_repository(&fixture.0).unwrap(), "release", "modes").unwrap();
    assert!(git(&fixture.0, &["ls-tree", "HEAD", "script"]).starts_with("100755"));
    assert!(git(&fixture.0, &["ls-tree", "HEAD", "link"]).starts_with("120000"));
}
