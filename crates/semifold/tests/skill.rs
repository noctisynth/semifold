#![allow(clippy::unwrap_used)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

struct Directory(PathBuf);

impl Directory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "semifold-skill-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_smif"))
        .args(arguments)
        .current_dir(root)
        .env("CHANGESET_PATH", root.join("missing-config-directory"))
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_TOKEN", "synthetic-unusable-token")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn response(output: Output, language: &str) -> Value {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["protocolVersion"], 1);
    assert_eq!(value["cliVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["language"], language);
    value["data"].clone()
}

#[test]
fn discovery_is_offline_and_independent_of_repository_and_configuration() {
    let root = Directory::new("discovery");
    let list = response(run(&root.0, &["skill", "list", "--language", "en"]), "en");
    let tasks = list.as_array().unwrap();
    assert!(!tasks.is_empty());
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);

    fs::write(root.0.join(".git"), "not a git directory").unwrap();
    fs::create_dir(root.0.join(".changes")).unwrap();
    fs::write(root.0.join(".changes/config.toml"), "[invalid TOML").unwrap();
    let repeated = response(run(&root.0, &["skill", "list", "--language", "en"]), "en");
    assert_eq!(list, repeated);
    let chinese_list = response(run(&root.0, &["skill", "list", "--language", "zh"]), "zh");
    let english_locale = include_str!("../locales/en.toml")
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    let chinese_locale = include_str!("../locales/zh.toml")
        .parse::<toml_edit::DocumentMut>()
        .unwrap();

    for task in tasks {
        let id = task["task"].as_str().unwrap();
        let english = response(
            run(&root.0, &["skill", "--language", "en", "get", id]),
            "en",
        );
        let chinese = response(
            run(&root.0, &["skill", "get", id, "--language", "zh"]),
            "zh",
        );
        assert_eq!(english["task"], id);
        assert_eq!(chinese["task"], id);
        assert_eq!(english["commands"], chinese["commands"]);
        assert_ne!(english["instructions"], chinese["instructions"]);
        for (source, summaries, guide) in [
            (&english_locale, &list, &english),
            (&chinese_locale, &chinese_list, &chinese),
        ] {
            let expected = &source["cli"]["skill"]["guides"];
            let summary_key = format!("{id}_summary");
            let summary = summaries
                .as_array()
                .unwrap()
                .iter()
                .find(|summary| summary["task"] == id)
                .unwrap();
            assert_eq!(
                summary["summary"],
                expected[summary_key.as_str()].as_str().unwrap()
            );
            assert_eq!(
                guide["instructions"],
                format!(
                    "{}\n\n{}",
                    expected["shared"].as_str().unwrap().trim_end(),
                    expected[id].as_str().unwrap()
                )
            );
        }
        for command in english["commands"].as_array().unwrap() {
            let mut arguments: Vec<_> = command.as_str().unwrap().split_whitespace().collect();
            arguments.push("--help");
            let output = run(&root.0, &arguments);
            assert!(output.status.success(), "{command}: {output:?}");
        }
    }
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 2);
}

#[test]
fn unknown_tasks_and_invalid_languages_fail_without_json_or_mutations() {
    let root = Directory::new("unknown-task");
    for language in ["en", "zh"] {
        let output = run(
            &root.0,
            &[
                "skill",
                "get",
                "missing-synthetic-task",
                "--language",
                language,
            ],
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("missing-synthetic-task"));
        assert!(error.contains("smif skill list"));
        assert!(!error.contains("cli.skill.errors."));
    }
    let output = run(&root.0, &["skill", "list", "--language", "unknown"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
}

#[test]
fn export_is_a_single_entrypoint_and_preserves_existing_targets() {
    let root = Directory::new("export");
    let exported = response(
        run(
            &root.0,
            &[
                "skill",
                "export",
                "--output",
                "semifold",
                "--language",
                "en",
            ],
        ),
        "en",
    );
    assert_eq!(exported["written"], true);
    assert_eq!(exported["files"], serde_json::json!(["SKILL.md"]));
    let target = root.0.join("semifold");
    assert_eq!(Path::new(exported["output"].as_str().unwrap()), target);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
    let entry = fs::read_to_string(target.join("SKILL.md")).unwrap();
    assert_eq!(entry, include_str!("../assets/skill/SKILL.md"));
    assert_eq!(
        entry.lines().take(2).collect::<Vec<_>>(),
        ["---", "name: semifold"]
    );

    fs::write(target.join("user-note.txt"), "preserve this file").unwrap();
    for arguments in [
        vec!["skill", "export", "--output", "semifold"],
        vec!["skill", "export", "--output", "semifold", "--dry-run"],
    ] {
        let output = run(&root.0, &arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read_to_string(target.join("SKILL.md")).unwrap(), entry);
        assert_eq!(
            fs::read_to_string(target.join("user-note.txt")).unwrap(),
            "preserve this file"
        );
    }
    fs::write(root.0.join("existing-file"), "keep").unwrap();
    let output = run(&root.0, &["skill", "export", "--output", "existing-file"]);
    assert!(!output.status.success());
    assert_eq!(
        fs::read_to_string(root.0.join("existing-file")).unwrap(),
        "keep"
    );
}

#[test]
fn export_dry_run_and_missing_parents_do_not_create_files() {
    let root = Directory::new("export-preview");
    let preview = response(
        run(
            &root.0,
            &[
                "--dry-run",
                "skill",
                "export",
                "--output",
                "preview",
                "--language",
                "zh",
            ],
        ),
        "zh",
    );
    assert_eq!(preview["written"], false);
    assert_eq!(preview["files"], serde_json::json!(["SKILL.md"]));
    assert!(!root.0.join("preview").exists());
    let output = run(&root.0, &["skill", "export", "--output", "missing/skill"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn export_preserves_link_targets_and_resolves_linked_parents() {
    use std::os::unix::fs::symlink;

    let root = Directory::new("export-links");
    symlink(root.0.join("absent"), root.0.join("dangling")).unwrap();
    let output = run(&root.0, &["skill", "export", "--output", "dangling"]);
    assert!(!output.status.success());
    assert!(fs::symlink_metadata(root.0.join("dangling")).is_ok());
    fs::create_dir(root.0.join("real")).unwrap();
    symlink(root.0.join("real"), root.0.join("linked")).unwrap();
    let output = run(&root.0, &["skill", "export", "--output", "linked"]);
    assert!(!output.status.success());
    assert_eq!(fs::read_dir(root.0.join("real")).unwrap().count(), 0);
    let exported = response(
        run(
            &root.0,
            &[
                "skill",
                "export",
                "--output",
                "linked/skill",
                "--language",
                "en",
            ],
        ),
        "en",
    );
    assert_eq!(exported["written"], true);
    assert_eq!(
        Path::new(exported["output"].as_str().unwrap()),
        root.0.join("real/skill")
    );
    assert_eq!(
        fs::read_to_string(root.0.join("real/skill/SKILL.md")).unwrap(),
        include_str!("../assets/skill/SKILL.md")
    );
}

#[cfg(unix)]
#[test]
fn export_rejects_non_unicode_canonical_paths_before_writing() {
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::symlink},
    };

    let root = Directory::new("export-path-encoding");
    let raw_parent = root.0.join(OsString::from_vec(b"parent-\xff".to_vec()));
    fs::create_dir(&raw_parent).unwrap();
    symlink(&raw_parent, root.0.join("alias")).unwrap();
    let output = run(&root.0, &["skill", "export", "--output", "alias/skill"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_dir(&raw_parent).unwrap().count(), 0);
}
