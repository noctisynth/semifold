#![allow(clippy::unwrap_used)]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use insta::assert_snapshot;
use semifold_core::{
    Dependency, DependencySource, EcosystemId, PackageId, PackageSnapshot, VersionMap,
    VersionSource, WorkspaceGraph,
};
use semifold_resolver::{
    adapter::{EcosystemAdapter, PackageInspection, PackageLocation},
    resolver::{
        cpp::CppResolver, nodejs::NodejsResolver, python::PythonResolver, rust::RustResolver,
    },
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn temp_dir(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "semifold-resolver-fixture-{test_name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn fixture(path: &str) -> PathBuf {
    Path::new(FIXTURES).join(path)
}

fn copy_fixture(source: &str, destination: &Path) {
    let source = fixture(source);
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::copy(source, destination).unwrap();
}

fn snapshot(path: &str, name: &str, ecosystem: EcosystemId) -> PackageSnapshot {
    PackageSnapshot {
        id: PackageId::new(name),
        manifest_name: name.to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        version_source: VersionSource::PackageManifest,
        ecosystem,
        path: path.into(),
        publishable: true,
        dependencies: vec![],
    }
}

fn render_packages(mut packages: Vec<PackageInspection>) -> String {
    packages.sort_by(|left, right| left.manifest_name.cmp(&right.manifest_name));
    packages
        .into_iter()
        .map(|package| {
            format!(
                "name = {}\nversion = {}\npath = {}\nprivate = {}",
                package.manifest_name,
                package.version,
                package
                    .path
                    .components()
                    .map(|component| component.as_str())
                    .collect::<Vec<_>>()
                    .join("/"),
                !package.publishable
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn discover(adapter: &dyn EcosystemAdapter, root: &Path) -> Vec<PackageInspection> {
    adapter
        .discover(camino::Utf8Path::from_path(root).unwrap())
        .unwrap()
}

fn inspect(adapter: &dyn EcosystemAdapter, root: &Path, id: &str, path: &str) -> PackageInspection {
    adapter
        .inspect(&PackageLocation {
            id: PackageId::new(id),
            project_root: camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap(),
            path: path.into(),
        })
        .unwrap()
}

fn adapter_order(
    adapter: &dyn EcosystemAdapter,
    root: &Path,
    packages: &[(&str, &str)],
) -> Vec<PackageId> {
    let project_root = camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap();
    let inspections = packages
        .iter()
        .map(|(id, path)| {
            adapter
                .inspect(&PackageLocation {
                    id: PackageId::new(*id),
                    project_root: project_root.clone(),
                    path: (*path).into(),
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    let package_ids = inspections
        .iter()
        .map(|package| (package.manifest_name.clone(), package.id.clone()))
        .collect::<BTreeMap<_, _>>();
    let snapshots = inspections
        .into_iter()
        .map(|package| PackageSnapshot {
            id: package.id,
            manifest_name: package.manifest_name,
            version: package.version,
            version_source: package.version_source,
            ecosystem: package.ecosystem,
            path: package.path,
            publishable: package.publishable,
            dependencies: package
                .dependencies
                .into_iter()
                .filter_map(|dependency| {
                    package_ids
                        .get(&dependency.manifest_name)
                        .cloned()
                        .map(|package| Dependency {
                            package,
                            kind: dependency.kind,
                            requirement: dependency.requirement,
                            source: DependencySource::Manifest,
                        })
                })
                .collect(),
        })
        .collect();

    WorkspaceGraph::new(snapshots)
        .unwrap()
        .topological_order()
        .unwrap()
}

#[test]
fn rust_manifest_parsing_matches_snapshot() {
    let root = temp_dir("rust-parse");
    copy_fixture("rust/workspace.Cargo.toml", &root.join("Cargo.toml"));
    copy_fixture("rust/core.Cargo.toml", &root.join("crates/core/Cargo.toml"));
    copy_fixture("rust/app.Cargo.toml", &root.join("crates/app/Cargo.toml"));

    assert_snapshot!(
        "rust_manifest_parsing",
        render_packages(discover(&RustResolver, &root))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn node_manifest_parsing_matches_snapshot() {
    let root = temp_dir("node-parse");
    let manifest = root.join("package.json");
    copy_fixture("node/app.before.json", &manifest);

    assert_snapshot!(
        "node_manifest_parsing",
        render_packages(vec![inspect(&NodejsResolver, &root, "app", ".")])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn python_manifest_parsing_matches_snapshot() {
    let root = temp_dir("python-parse");
    let manifest = root.join("pyproject.toml");
    copy_fixture("python/app.before.toml", &manifest);

    assert_snapshot!(
        "python_manifest_parsing",
        render_packages(vec![inspect(&PythonResolver, &root, "app", ".")])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cpp_manifest_parsing_matches_snapshot() {
    let root = temp_dir("cpp-parse");
    let manifest = root.join("CMakeLists.txt");
    copy_fixture("cpp/CMakeLists.before.txt", &manifest);

    assert_snapshot!(
        "cpp_manifest_parsing",
        render_packages(vec![inspect(&CppResolver, &root, "example", ".")])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cpp_workspace_fixture_discovers_members_and_orders_dependencies() {
    let root = temp_dir("cpp-workspace");
    copy_fixture("cpp/workspace.CMakeLists.txt", &root.join("CMakeLists.txt"));
    copy_fixture(
        "cpp/core.CMakeLists.txt",
        &root.join("libraries/core/CMakeLists.txt"),
    );
    copy_fixture(
        "cpp/app.CMakeLists.txt",
        &root.join("applications/app/CMakeLists.txt"),
    );

    assert_snapshot!(
        "cpp_workspace_manifest_parsing",
        render_packages(discover(&CppResolver, &root))
    );

    assert_eq!(
        adapter_order(
            &CppResolver,
            &root,
            &[("app", "applications/app"), ("core", "libraries/core")]
        ),
        [PackageId::new("core"), PackageId::new("app")]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rust_manifest_rewrite_matches_snapshot() {
    let root = temp_dir("rust-golden");
    let manifest = root.join("crates/app/Cargo.toml");
    copy_fixture("rust/app.before.toml", &manifest);

    let app = snapshot("crates/app", "app", EcosystemId::RUST);
    let core = snapshot("crates/core", "core", EcosystemId::RUST);
    let edits = RustResolver::plan_file_edits(
        &root,
        &[&app],
        &[&app, &core],
        &VersionMap::from([
            (
                PackageId::new("app"),
                semver::Version::parse("1.0.1").unwrap(),
            ),
            (
                PackageId::new("core"),
                semver::Version::parse("1.1.0").unwrap(),
            ),
        ]),
    )
    .unwrap();
    fs::write(&manifest, &edits[0].new_content).unwrap();

    assert_snapshot!(
        "rust_manifest_rewrite",
        fs::read_to_string(manifest).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn node_manifest_rewrite_matches_snapshot() {
    let root = temp_dir("node-golden");
    let manifest = root.join("packages/app/package.json");
    copy_fixture("node/app.before.json", &manifest);

    let edit = NodejsResolver::plan_file_edit(
        &root,
        &snapshot("packages/app", "app", EcosystemId::NODE),
        &VersionMap::from([(
            PackageId::new("app"),
            semver::Version::parse("1.0.1").unwrap(),
        )]),
    )
    .unwrap();
    fs::write(&manifest, edit.new_content).unwrap();

    assert_snapshot!(
        "node_manifest_rewrite",
        fs::read_to_string(manifest).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn python_manifest_rewrite_matches_snapshots() {
    let root = temp_dir("python-golden");
    let manifest = root.join("packages/app/pyproject.toml");
    let init = root.join("packages/app/src/app/__init__.py");
    copy_fixture("python/app.before.toml", &manifest);
    copy_fixture("python/init.before.py", &init);

    let package = snapshot("packages/app", "app", EcosystemId::PYTHON);
    for edit in PythonResolver::plan_file_edits(
        &root,
        &package,
        &VersionMap::from([(
            PackageId::new("app"),
            semver::Version::parse("1.0.1").unwrap(),
        )]),
    )
    .unwrap()
    {
        fs::write(root.join(edit.path.as_std_path()), edit.new_content).unwrap();
    }

    assert_snapshot!(
        "python_manifest_rewrite",
        fs::read_to_string(manifest).unwrap()
    );
    assert_snapshot!(
        "python_static_version_rewrite",
        fs::read_to_string(init).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cpp_manifest_rewrite_matches_snapshots() {
    let root = temp_dir("cpp-golden");
    let cmake = root.join("CMakeLists.txt");
    let vcpkg = root.join("vcpkg.json");
    copy_fixture("cpp/CMakeLists.before.txt", &cmake);
    copy_fixture("cpp/vcpkg.before.json", &vcpkg);

    let package = snapshot(".", "example", EcosystemId::CPP);
    for edit in CppResolver::plan_file_edits(
        &root,
        &package,
        &VersionMap::from([(
            PackageId::new("example"),
            semver::Version::parse("1.0.1").unwrap(),
        )]),
    )
    .unwrap()
    {
        fs::write(root.join(edit.path.as_std_path()), edit.new_content).unwrap();
    }

    assert_snapshot!(
        "cpp_cmake_manifest_rewrite",
        fs::read_to_string(cmake).unwrap()
    );
    assert_snapshot!(
        "cpp_vcpkg_manifest_rewrite",
        fs::read_to_string(vcpkg).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rust_workspace_dependency_fixture_is_discovered_and_retained() {
    let root = temp_dir("rust-workspace-dependency");
    copy_fixture("rust/workspace.Cargo.toml", &root.join("Cargo.toml"));
    copy_fixture("rust/core.Cargo.toml", &root.join("crates/core/Cargo.toml"));
    copy_fixture("rust/app.Cargo.toml", &root.join("crates/app/Cargo.toml"));

    let mut packages = discover(&RustResolver, &root);
    packages.sort_by(|left, right| left.manifest_name.cmp(&right.manifest_name));
    assert_eq!(
        packages
            .iter()
            .map(|package| &package.manifest_name)
            .collect::<Vec<_>>(),
        vec!["app", "core"]
    );

    let dependencies = inspect(&RustResolver, &root, "app", "crates/app")
        .dependencies
        .into_iter()
        .map(|dependency| dependency.manifest_name)
        .collect::<Vec<_>>();
    assert_eq!(dependencies, vec!["core"]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rust_single_and_private_package_fixtures_match_snapshot() {
    let root = temp_dir("rust-single-private");
    copy_fixture("rust/single.Cargo.toml", &root.join("single/Cargo.toml"));
    copy_fixture("rust/private.Cargo.toml", &root.join("private/Cargo.toml"));

    assert_snapshot!(
        "rust_single_and_private_packages",
        render_packages(vec![
            inspect(&RustResolver, &root, "single", "single"),
            inspect(&RustResolver, &root, "private", "private"),
        ])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn node_npm_and_pnpm_workspace_fixtures_match_snapshot() {
    let root = temp_dir("node-workspaces");
    copy_fixture("node/npm.root.package.json", &root.join("npm/package.json"));
    copy_fixture(
        "node/core.package.json",
        &root.join("npm/packages/core/package.json"),
    );
    copy_fixture(
        "node/private.package.json",
        &root.join("npm/packages/private/package.json"),
    );
    copy_fixture(
        "node/pnpm.root.package.json",
        &root.join("pnpm/package.json"),
    );
    copy_fixture(
        "node/pnpm-workspace.yaml",
        &root.join("pnpm/pnpm-workspace.yaml"),
    );
    copy_fixture(
        "node/core.package.json",
        &root.join("pnpm/packages/core/package.json"),
    );
    copy_fixture(
        "node/private.package.json",
        &root.join("pnpm/packages/private/package.json"),
    );

    assert_snapshot!(
        "node_npm_workspace_parsing",
        render_packages(discover(&NodejsResolver, &root.join("npm")))
    );
    assert_snapshot!(
        "node_pnpm_workspace_parsing",
        render_packages(discover(&NodejsResolver, &root.join("pnpm")))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn node_peer_dependency_fixture_orders_dependency_before_dependent() {
    let root = temp_dir("node-peer-dependency");
    copy_fixture(
        "node/core.package.json",
        &root.join("packages/core/package.json"),
    );
    copy_fixture(
        "node/peer-app.package.json",
        &root.join("packages/app/package.json"),
    );

    assert_eq!(
        adapter_order(
            &NodejsResolver,
            &root,
            &[("app", "packages/app"), ("core", "packages/core")]
        ),
        [PackageId::new("core"), PackageId::new("app")]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn python_metadata_and_monorepo_fixtures_match_snapshot() {
    let root = temp_dir("python-monorepo");
    copy_fixture("python/root.pyproject.toml", &root.join("pyproject.toml"));
    copy_fixture(
        "python/pep.pyproject.toml",
        &root.join("packages/pep/pyproject.toml"),
    );
    copy_fixture(
        "python/poetry.pyproject.toml",
        &root.join("libs/poetry/pyproject.toml"),
    );
    copy_fixture(
        "python/hatch.pyproject.toml",
        &root.join("apps/hatch/pyproject.toml"),
    );
    copy_fixture(
        "python/hatch.init.py",
        &root.join("apps/hatch/src/hatch/__init__.py"),
    );
    copy_fixture("python/setup.cfg", &root.join("packages/cfg/setup.cfg"));

    assert_snapshot!(
        "python_monorepo_manifest_parsing",
        render_packages(discover(&PythonResolver, &root))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn node_dependency_fixture_orders_dependency_before_dependent() {
    let root = temp_dir("node-dependencies");
    copy_fixture(
        "node/core.package.json",
        &root.join("packages/core/package.json"),
    );
    copy_fixture(
        "node/app.package.json",
        &root.join("packages/app/package.json"),
    );

    assert_eq!(
        adapter_order(
            &NodejsResolver,
            &root,
            &[("app", "packages/app"), ("core", "packages/core")]
        ),
        [PackageId::new("core"), PackageId::new("app")]
    );
    fs::remove_dir_all(root).unwrap();
}
