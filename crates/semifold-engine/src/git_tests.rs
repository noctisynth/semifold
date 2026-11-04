use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn strict_trust_uses_only_protected_configuration_and_honors_resets() {
    let sandbox = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "semifold-git-trust-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let root = sandbox.join("trusted space 中文/nested/repo");
    fs::create_dir_all(&root).unwrap();
    gix::init(&root).unwrap();
    let config = sandbox.join("system.gitconfig");
    let prefix = sandbox
        .join("trusted space 中文")
        .to_string_lossy()
        .replace('\\', "/");
    let exact = root.to_string_lossy().replace('\\', "/");
    let sibling = sandbox.join("trusted space 中文-other/repo");
    fs::create_dir_all(&sibling).unwrap();
    gix::init(&sibling).unwrap();
    // A repository cannot grant itself trust.
    fs::write(root.join(".git/config"), "[safe]\n directory = *\n").unwrap();
    for (values, allowed) in [
        (vec![format!("{prefix}/*")], true),
        (vec![exact.clone()], true),
        (vec!["*".into()], true),
        (vec![], false),
        (vec![format!("{prefix}/*"), String::new()], false),
        (vec![String::new(), exact], true),
    ] {
        let text = format!(
            "[safe]\n{}",
            values
                .iter()
                .map(|value| format!(" directory = {value}\n"))
                .collect::<String>()
        );
        fs::write(&config, text).unwrap();
        let options = options_with_config(&config);
        let opened = gix::open_opts(&root, options.clone());
        assert_eq!(opened.is_ok(), allowed, "values={values:?}: {opened:?}");
        let discovered = gix::discover_opts(
            &root,
            gix::discover::upwards::Options {
                trust: gix::discover::upwards::TrustPolicy::Assume(gix::sec::Trust::Reduced),
                ..Default::default()
            },
            options,
        );
        assert_eq!(
            discovered.is_ok(),
            allowed,
            "discover values={values:?}: {discovered:?}"
        );
        if !allowed {
            assert!(opened.unwrap_err().is_permission_denied());
        }
    }
    fs::write(&config, format!("[safe]\n directory = {prefix}/*\n")).unwrap();
    assert!(
        gix::open_opts(&sibling, options_with_config(&config))
            .unwrap_err()
            .is_permission_denied()
    );
    // gix treats the prefix directory itself as part of its subtree. Record this
    // explicit backend boundary instead of implying byte-for-byte Git parity.
    gix::init(sandbox.join("trusted space 中文")).unwrap();
    assert!(
        gix::open_opts(
            sandbox.join("trusted space 中文"),
            options_with_config(&config)
        )
        .is_ok()
    );
    fs::remove_dir_all(sandbox).unwrap();
}

fn options_with_config(path: &std::path::Path) -> gix::open::Options {
    let mut permissions = gix::open::Permissions::default();
    permissions.config = gix::open::permissions::Config::isolated();
    permissions.config.system = true;
    // Force reduced trust only in this test, independently of filesystem owners.
    // The normal strict production options and gix's real protected config reader
    // still perform the authorization; no matching rules are reimplemented here.
    open_options()
        .permissions(permissions)
        .system_config_path(path)
        .with(gix::sec::Trust::Reduced)
}
