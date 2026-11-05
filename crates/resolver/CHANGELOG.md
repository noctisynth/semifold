# Changelog

## v0.4.3

### Bug Fixes

- [`bcde56a`](https://github.com/noctisynth/semifold/commit/bcde56aeb7737dd71f4a4f948f265f934c98214b): Use official gix for all local Git operations, including directory-wildcard trust, worktree status, changeset history, and CI release commits. Keep libgit2 isolated in the CI push adapter, preflight it before release writes, and preserve Git error diagnostics in the CLI and MCP. Local operations do not require an installed Git executable; CI push retains libgit2's directory-wildcard limitation. Changelog collection and engine Git errors now expose gix types, and history lookup returns errors explicitly. ([#118](https://github.com/noctisynth/semifold/pull/118) by @fu050409)

    Keep plugin file-list protocol paths slash-separated on Windows so discovered manifests can be read through the same scoped host API.


### Chores

- [`dbf3de7`](https://github.com/noctisynth/semifold/commit/dbf3de77f994a5ba092c419661f7e0013765c993): Refresh official stable Rust and Bun workspace dependencies and lockfiles, including Boa 0.22, MiniJinja 3, saphyr 0.3, N-API, and the documentation toolchain. Adapt explicit template serialization, YAML nodes, fallible Promise resolution, and MCP server configuration while preserving plugin permissions, resource budgets, strict templates, protocol schema v1, and the local gix / git2 push boundary. Migrate Biome configuration with its official CLI. ([#118](https://github.com/noctisynth/semifold/pull/118) by @fu050409)

    Regenerate the N-API loader and declarations using the upgraded official CLI and verify the Windows native module.

## v0.4.2

### Bug Fixes

- [`c47ccd5`](https://github.com/noctisynth/semifold/commit/c47ccd5786ab38391efc8ca09cd3ab8cd4477960): Reject release branches that match the base branch during configuration loading, initialization, and rendered release execution before Semifold can force-update the base branch.

## v0.4.1

### Chores

- [`05a2e7c`](https://github.com/noctisynth/semifold/commit/05a2e7c6b85697cbd091f74698d88a87f8678f01): Upgrade Rust and JavaScript dependencies to their latest compatible versions, including the required API migrations for Git, HTTP, hashing, localization, MCP, and configuration handling.

### New Features

- [`3454415`](https://github.com/noctisynth/semifold/commit/34544159c278e1c9096e1e002ab6966f978300e6): Allow release commit messages and pull request titles to use optional strict release context templates while preserving the existing defaults.

## v0.4.0

### New Features

- [`c64a001`](https://github.com/noctisynth/semifold/commit/c64a0011732bb40044d9486ef708fb03bfc9c650): Add an optional package-level `publish` override that defaults to manifest or plugin discovery and explicitly controls registry pre-check and publish command eligibility.

## v0.4.0-rc.4

### Chores

- [`9632bac`](https://github.com/noctisynth/semifold/commit/9632bacee5412b6a37103a68b8db1e7b8e86909a): Migrate the repository JavaScript workspace, SDK checks, runtime tests, documentation build, and CI/CD dependency installation from pnpm to Bun canary with a committed `bun.lock`.

## v0.4.0-rc.3

### Bug Fixes

- [`0c59f66`](https://github.com/noctisynth/semifold/commit/0c59f66364e6617e9e711c21f217dbff256ffcdf): Introduce the versioned TypeScript SDK for ecosystem plugins with exact schema v1 wire types, construction helpers, and declarations limited to the capabilities provided by the Boa runtime.

    Build and validate the public package without Node.js or DOM ambient types, share JSON contract fixtures with the Rust protocol tests, and prepare OIDC-only automated npm publishing after the initial version is published locally.

- [`5b6694b`](https://github.com/noctisynth/semifold/commit/5b6694bbd17d86f9f3b44977ea402849d26228a3): Format generated plugin SDK bindings with Biome before writing or drift checks, and enforce SDK formatting in CI.
- [`6b451f6`](https://github.com/noctisynth/semifold/commit/6b451f614df71cb1618c81dc02ff3e37953e4c6f): Generate plugin SDK wire types from the Rust serde protocol, preserve deeply readonly public types, and fail CI when committed bindings drift.

    Serialize workspace-manifest edit source fields with the documented kebab-case names.


### New Features

- [`5496c96`](https://github.com/noctisynth/semifold/commit/5496c96f4a5b26626ac7267b64b81c96fafc0b6b): Adapt authenticated JavaScript plugins directly to the existing ecosystem boundary, with deterministic snapshot conversion and host-side validation of project roots, package identities, paths, edit sources, duplicate targets, and streamed file hashes.
- [`3a4e1c7`](https://github.com/noctisynth/semifold/commit/3a4e1c7ea2d89e1b6de33c50759330ffccbff2d6): Expose frozen asynchronous file capabilities to Boa plugins, with declared-glob authorization, project-root and symlink isolation, deterministic listings, UTF-8 enforcement, and per-operation path and byte budgets.
- [`ca56371`](https://github.com/noctisynth/semifold/commit/ca56371c162e7845e8f8652d9e2b488021a85de8): Configure repository-local dynamic ecosystem plugins by stable ID with optional SHA-256 pins and exact HTTPS origins, and route discovery, workspace loading, config sync, and version edit planning through their authenticated adapters.
- [`493c256`](https://github.com/noctisynth/semifold/commit/493c256206ed20708580ced5ebd8a379dae055ec): Embed Boa as the JavaScript plugin runtime with single-file ESM execution, protocol validation, fixed VM budgets, and a deny-by-default injectable fetch backend.
- [`dd10f1a`](https://github.com/noctisynth/semifold/commit/dd10f1a00b2a99219861083ba3fc10d07322fe72): Introduce validated, stable ecosystem identities and a schema-versioned plugin protocol for metadata, discovery, inspection, edit planning, and structured diagnostics.
- [`ed27de8`](https://github.com/noctisynth/semifold/commit/ed27de8c19b2da349ed9a3d104ca4a9fedb27923): Load repository-local JavaScript plugins through a deterministic ecosystem registry that verifies normalized paths, SHA-256 content locks, source limits, metadata identities, and project-scoped file capabilities before execution.
- [`90f27bb`](https://github.com/noctisynth/semifold/commit/90f27bb8bd85a7d0a1dbb6830cfec2ed46ec1152): Replace the MCP changeset surface with lazily loaded, structured get/create/update/delete tools, optimistic SHA-256 revisions, dry-run planning, localized errors, and panic isolation that keeps the stdio server available after failed calls.
- [`8caff0a`](https://github.com/noctisynth/semifold/commit/8caff0ad5ace8ca72600ed1acabd08a6a1ec2b13): Migrate domain packages, plans, contexts, and adapters to open ecosystem identities while preserving built-in serialization and ordering compatibility ahead of dynamic plugin registration.
- [`3e5a8c1`](https://github.com/noctisynth/semifold/commit/3e5a8c127c6a3ccb66e13f6ce8aad474d276aa03): Add a default-deny, exact-HTTPS-origin fetch capability for plugins with per-hop redirect validation, public-address-only DNS resolution, an injectable asynchronous transport, and per-operation request, concurrency, and body budgets.

## v0.4.0-rc.2

### Bug Fixes

- [`dc12a58`](https://github.com/noctisynth/semifold/commit/dc12a58bbd728e84196d0aaef27a61cf76467d3a): Reject malformed changeset separators, empty package lists, and empty summaries while loading
    changesets so status reports invalid input before changelog rendering.

### New Features

- [`c90d417`](https://github.com/noctisynth/semifold/commit/c90d4173d28e04e951183b3f10bc28905df0df2a): Make HTTP publish pre-checks inject an overridable runtime User-Agent, retry transient failures
    using configured delays and Retry-After, and report bounded response details for registry errors.
- [`7ca5d48`](https://github.com/noctisynth/semifold/commit/7ca5d48f26c5de2f888d89c4b45d8650165b9b74): Limit Semifold configuration to TOML and add typed HTTP or command publish pre-checks. Command
    pre-checks exchange package metadata and existence results through a strict JSON Lines protocol,
    while HTTP checks now fail safely on statuses other than 200 and 404.

## v0.4.0-rc.1

### New Features

- [`2f843f3`](https://github.com/noctisynth/semifold/commit/2f843f38b196bf61cffd41daa6ae07cdccd6fe94): Allow workspace owners to customize complete changelog release blocks and individual changeset entries with strict MiniJinja templates. Template contexts now expose structured summaries and commit metadata, while stable release markers let publish consume arbitrary rendered formats without breaking legacy changelogs.
- [`42f88c9`](https://github.com/noctisynth/semifold/commit/42f88c9302f8d974c8351aaa30297b0aff2fb002): Write the built-in release and changeset changelog templates into newly initialized configuration files so users can discover and customize them immediately, while retaining the same templates as fallbacks for older configurations.
- [`145ccec`](https://github.com/noctisynth/semifold/commit/145ccecb196b51bbe40d1123c6b7ad6f4678aa25): Add an optional per-package `github-release` policy. Public packages keep GitHub Releases enabled by
    default, private packages keep them disabled by default, and either default can now be overridden
    explicitly without changing registry publishability.

## v0.4.0-rc.0

### Bug Fixes

- [`6838d72`](https://github.com/noctisynth/semifold/commit/6838d72730fc38e389885322637621bba0d2aadd): Introduce explicit domain and application error boundaries and remove `anyhow` from the engine.

    All production targets now reject panic-prone unwraps, expects, indexing, and slicing under strict
    Clippy validation, including workspace planning, Rust manifest edits, changelog metadata parsing,
    configuration editing, and embedded initialization assets.


### Refactors

- [`d50a156`](https://github.com/noctisynth/semifold/commit/d50a156035a6520442c0bbd44923d5ac2b36f6b1): Move project loading, configuration synchronization, release planning, changelog preparation, and
    release application behind `SemifoldService` and the new `semifold-engine` boundary.

    CLI and CI now share an immutable `ReleasePlan` followed by a complete `ReleaseApplyPlan`, MCP no
    longer changes the process working directory, and the legacy global mutable `Context` is removed.

## v0.4.0-beta.3

### New Features

- [`ea86a6a`](https://github.com/noctisynth/semifold/commit/ea86a6a87bb6b155d13e1aceabe9cf7f3da974fd): Support Rust packages that inherit `workspace.package.version`.

    Shared version sources now merge bumps across every inheriting crate, validate channel policy, keep
    private crates in the version closure, and update the owning workspace manifest exactly once.


### Refactors

- [`ea86a6a`](https://github.com/noctisynth/semifold/commit/ea86a6a87bb6b155d13e1aceabe9cf7f3da974fd): Use kebab-case for every Semifold configuration field.

    Snake-case configuration keys are no longer supported. Repository configuration, generated
    configuration, and fixtures now use fields such as `dry-run`, `extra-env`, and `extra-headers`.

## v0.4.0-beta.2

### New Features

- [`e8f8a09`](https://github.com/noctisynth/semifold/commit/e8f8a0966142aacdab13a33d41332fd9612e4cf9): Add one-shot channel transition bump overrides to `config channel set`, including a preserve mode for entering prerelease channels without raising the stable version base.

## v0.4.0-beta.1

### New Features

- [`e015e6d`](https://github.com/noctisynth/semifold/commit/e015e6d234f97b87e7422431b4b3e659a3a218df): Validate ecosystem release channels and encode Python PEP 440 pre- and post-release versions.
- [`e108ed0`](https://github.com/noctisynth/semifold/commit/e108ed0721d928d8ca543ce7bc3c00db0030afe3): Support explicit cross-ecosystem dependency ordering and release propagation.

### Refactors

- [`0dab79f`](https://github.com/noctisynth/semifold/commit/0dab79fe5ac1ba645820c831e4605da4b1aa7a1f): Move publish command execution and dry-run handling out of ecosystem resolvers.
- [`8a65d5a`](https://github.com/noctisynth/semifold/commit/8a65d5aceb50ff15ca7acede9dfffe37de9faf75): Remove legacy resolver-local package sorting and verify dependency order through the unified WorkspaceGraph topology.
- [`f74c91a`](https://github.com/noctisynth/semifold/commit/f74c91a25ae2fe97f4657a77f3ac1a5f1156a2f8): Converge ecosystem discovery, inspection, publishing, and fixture coverage on the EcosystemAdapter interface.

## v0.4.0-beta.0

### Refactors

- [`0d9f35d`](https://github.com/noctisynth/semifold/commit/0d9f35d368a5b3a84ce93ba20c9dc4dcf96a5090): Route Rust package discovery, inspection, and release edit planning through the side-effect-free ecosystem adapter boundary.
- [`a88431b`](https://github.com/noctisynth/semifold/commit/a88431bcca5a4fa60b05b0fd291ec3781f7b906d): Route C++ package discovery, inspection, and CMake/vcpkg edit planning through the ecosystem adapter with recursive in-root workspace discovery.
- [`31a668f`](https://github.com/noctisynth/semifold/commit/31a668f13b31501f406f2c08f32f012780537231): Route Python package discovery, inspection, and native version file planning through the ecosystem adapter while preserving read-only Cargo version sources.
- [`9e6b717`](https://github.com/noctisynth/semifold/commit/9e6b7170b2b75f44d26b6d5aa80917daa63149ff): Route Node.js package discovery, inspection, and release edit planning through the ecosystem adapter boundary with configured package id support.

## v0.4.0-alpha.7

### Bug Fixes

- [`fce12df`](https://github.com/noctisynth/semifold/commit/fce12dff6bf2282af11f9129d3620ef852daf1e8): Plan Rust package and workspace dependency version edits as one deterministic batch.

### New Features

- [`38e7011`](https://github.com/noctisynth/semifold/commit/38e7011afb86e2a2cf04c9bb90f1b258bda8df87): Define the side-effect-free ecosystem adapter contract and complete batch edit planning input.
- [`04e0a5e`](https://github.com/noctisynth/semifold/commit/04e0a5e5da84a3e5703ba9eaa839a54b17f178ad): Plan Python manifest and source version edits through the unified release plan without modifying Cargo.toml.

## v0.4.0-alpha.6

### New Features

- [`419d267`](https://github.com/noctisynth/semifold/commit/419d267f7f15a2ff930f5fc9f8cd256735b8b07a): Plan CMake and vcpkg version edits before applying C++ release changes.

## v0.4.0-alpha.5

### Bug Fixes

- [`3b961e0`](https://github.com/noctisynth/semifold/commit/3b961e069ea4bd5b43e5b29bf2f5c5fc39414c9b): Return errors instead of panicking when release planning, configuration, changelog, and resolver invariants are unavailable.

### Refactors

- [`f42b195`](https://github.com/noctisynth/semifold/commit/f42b195058a4f6972f1a7468023c0512f4845d24): Keep recoverable input failures as errors while using documented `expect` calls for verified internal invariants.
- [`56e0686`](https://github.com/noctisynth/semifold/commit/56e06863fb2846497ed0b79417d68f3cf17eb8ca): Enforce the production unwrap policy through shared workspace Clippy configuration while allowing documented internal expects.
- [`09e7af6`](https://github.com/noctisynth/semifold/commit/09e7af6e76bed7a01b72e6a675504ab308732d2f): Plan Rust and Node changelog updates as validated file edits that can safely create a missing changelog.
- [`2014b6e`](https://github.com/noctisynth/semifold/commit/2014b6eb2139643e130b51b4ad7f0223f0e826a0): Remove Rust and Node.js legacy direct version writes in favor of planned file edits.

## v0.4.0-alpha.4

### New Features

- [`9136774`](https://github.com/noctisynth/semifold/commit/91367741414431f6e77bb2b3584e9a84b5c39cd7): Plan Rust manifest version replacements from the complete release VersionMap without writing files.
- [`e6ff3b7`](https://github.com/noctisynth/semifold/commit/e6ff3b7a90f8b6e93832ca7b9da6002c5b1dedd9): Plan Node.js package.json version and internal dependency replacements from the complete release VersionMap without rewriting unrelated JSON.

### Bug Fixes

- [`fc7284b`](https://github.com/noctisynth/semifold/commit/fc7284b604ed488a725f0222865946bea646eb3f): Serialize planned package.json edits with serde_json while preserving object-key order and a trailing newline.
- [`02dc67d`](https://github.com/noctisynth/semifold/commit/02dc67d11f54e1db0cc8a0c32855493218ff8556): Treat package.json files without a version field as version 0.0.0 and insert the planned version when writing.

## v0.4.0-alpha.3

### Bug Fixes

- [`3abdf29`](https://github.com/noctisynth/semifold/commit/3abdf29df5cbbe5542ff72a1f98c276dd74e4406): Add shared deterministic package discovery for init and config synchronization, and fail incomplete workspace scans instead of silently skipping invalid packages.

## v0.4.0-alpha.2

### New Features

- [`5bea426`](https://github.com/noctisynth/semifold/commit/5bea42624aa7ba73034ad0fba8dc1c9c14da7419): Bridge configured Rust, Node.js, Python, and C++ packages into the new cross-ecosystem workspace graph.

## v0.4.0-alpha.1

### New Features

- [`ea7c693`](https://github.com/noctisynth/semifold/commit/ea7c693f4b0c3f9e6c44200bebc13f3febedf159): Discover direct CMake workspace members and order internal target dependencies.

### Bug Fixes

- [`945657b`](https://github.com/noctisynth/semifold/commit/945657baa2b2dcae3bad60f59cba2d1ab1d66568): Only release Rust runtime dependents when their internal dependency version constraints are no longer satisfied.
- [`009d95d`](https://github.com/noctisynth/semifold/commit/009d95d59292f6db4cf7cf8bc2f486594cfeaa44): Preserve package.json and vcpkg.json formatting when updating package versions.

### Chores

- [`1860a03`](https://github.com/noctisynth/semifold/commit/1860a0304c443bfafdeeca349fbab3720c0ecd24): Add ecosystem manifest fixtures and snapshots for resolver regression coverage.

## v0.4.0-alpha.0

### Bug Fixes

- [`334977f`](https://github.com/noctisynth/semifold/commit/334977ff31af0b0a0858a82c1e9c383e5f333069): Automatically include transitive Rust dependents in version releases and rewrite their internal dependency versions before post-version commands run.
- [`8a9a56e`](https://github.com/noctisynth/semifold/commit/8a9a56ea402c1e4364c2ed210b3696a3d0b37f73): Fix formatting lint violations reported by Rust 1.97.

### New Features

- [`1f2498c`](https://github.com/noctisynth/semifold/commit/1f2498cf34d99663199d49a1ab9bbc7d88a34c1c): Add alpha release-channel lifecycle support, including stable-base selection, in-channel sequencing, and channel switching.

## v0.3.5

### Bug Fixes

- [`65a53a7`](https://github.com/noctisynth/semifold/commit/65a53a7a5e121f0fa52e258f14681aa727a473c9): Auto add version field for path based dependencies.

## v0.3.4

### Bug Fixes

- [`1eb7732`](https://github.com/noctisynth/semifold/commit/1eb7732b230b9c809e292f6ec3324e3eb7dfba34): Ensure all assets filtered by glob patterns are files.

## v0.3.3

### Bug Fixes

- [`df6e2ab`](https://github.com/noctisynth/semifold/commit/df6e2abd48beff959570d9cce997a7a00c829ee9): Always resolve asset files and use full path glob pattern instead.

## v0.3.2

### New Features

- [`fd41853`](https://github.com/noctisynth/semifold/commit/fd41853260fbb5b1e61a41373c24684d2a38e22e): Support search upload assets by glob pattern.

## v0.3.1

### Bug Fixes

- [`8a26838`](https://github.com/noctisynth/semifold/commit/8a2683871626a57a4e3b80788c8f151d8fde9a76): Fix rust private flag.
- [`306d737`](https://github.com/noctisynth/semifold/commit/306d7375fb2da7adabf9ad4b268e119674732c17): Fix resolver display names, use camel case instead.

## v0.3.0

### New Features

- [`d8959d0`](https://github.com/noctisynth/semifold/commit/d8959d02b980e2407fa95009e8afbf4c4375b1c0): 1. Add base_url field to RepoInfo struct 2. Read GITHUB_SERVER_URL env var with fallback to <https://github.com> 3. Use dynamic URL in changelog commit links ([#58](https://github.com/noctisynth/semifold/pull/58) by @BegoniaHe)

### Bug Fixes

- [`d8959d0`](https://github.com/noctisynth/semifold/commit/d8959d02b980e2407fa95009e8afbf4c4375b1c0): Convert `parts` from Vec<&str> to Vec<String> in bump_prerelease function ([#58](https://github.com/noctisynth/semifold/pull/58) by @BegoniaHe)
- [`d8959d0`](https://github.com/noctisynth/semifold/commit/d8959d02b980e2407fa95009e8afbf4c4375b1c0): Rust projects may not have a [dependencies] section (e.g., pure library crates or those with only dev-dependencies). This change makes the dependencies table optional instead of requiring it. ([#58](https://github.com/noctisynth/semifold/pull/58) by @BegoniaHe)

### Refactors

- [`d8959d0`](https://github.com/noctisynth/semifold/commit/d8959d02b980e2407fa95009e8afbf4c4375b1c0): 1. Add optional clap dependency to semifold-resolver 2. Conditionally derive ValueEnum on ResolverType 3. Remove duplicate ResolverType definition in init.rs ([#58](https://github.com/noctisynth/semifold/pull/58) by @BegoniaHe)

## v0.2.2

### New Features

- [`d39c94a`](https://github.com/noctisynth/semifold/commit/d39c94a3b30df9640fb147c77a820a87c9167319): bump version for semifold cpp support ([#52](https://github.com/noctisynth/semifold/pull/52) by @BegoniaHe)

## v0.2.1

### New Features

- [`45b6ab3`](https://github.com/noctisynth/semifold/commit/45b6ab314430aa410d44ad2c545518773d812337): Fix the order of JSON fields when bumping versions.

## v0.2.0

### Bug Fixes

- [`cd86453`](https://github.com/noctisynth/semifold/commit/cd86453841b6d394d9281c0412ad5e75794b85a9): Fix git repo status check for `git2::Status::IGNORE` files.
- [`8e8bf97`](https://github.com/noctisynth/semifold/commit/8e8bf97acefef3faf4817957acb12ec0f91dd93a): Fix glob pattern on non-posix platforms.

### Refactors

- [`86c97d9`](https://github.com/noctisynth/semifold/commit/86c97d9a63cff0931588c434908bcf4fe91f7805): Mark `--dry-run` flag as global options.

### New Features

- [`e009c7e`](https://github.com/noctisynth/semifold/commit/e009c7ec0d2908cdf6bf11430a7c0db46f8f40ad): Support running commands in dry run mode.
- [`98e4a7d`](https://github.com/noctisynth/semifold/commit/98e4a7d7ba33a1179fd542fdef0c7a4011ecab64): Sort packages and cache version bumps in version process, fix Rust workspace related packages version bump.
- [`27b53b2`](https://github.com/noctisynth/semifold/commit/27b53b28c15e7056f54e0f61ae8f688cf714e59a): When switching from pre-release mode to production mode, ignore minor and major version bumps and remove only the pre-release tag.

## v0.1.17

### Bug Fixes

- [`1862ba8`](https://github.com/noctisynth/semifold/commit/1862ba8d7df701893a65b9187cdbaf9ecaf20fa0): Fix version bump when version mode changed from pre-release to semantic.
- [`1cd6143`](https://github.com/noctisynth/semifold/commit/1cd6143b4b0c87cb33b42b085da8149262b5ef53): Nodejs resolver should include root package when resolving nodejs workspaces.

## v0.1.16

### New Features

- [`4856c7d`](https://github.com/noctisynth/semifold/commit/4856c7d14bb2bd3622f9ae29f8b75e5ad2f60165): Improve compatibility to `changesets` and `covector`, allow empty tag key now.

## v0.1.15

### New Features

- [`35dad5f`](https://github.com/noctisynth/semifold/commit/35dad5f2d1b5348b2740cd4269005f52b5ca599b): Support pre-release versioning mode.

## v0.1.14

### Bug Fixes

- [`eb80fa8`](https://github.com/noctisynth/semifold/commit/eb80fa8ad0cf07b522f6e0f95a55371893788424): Fix current Git repository status check.

### New Features

- [`4774f04`](https://github.com/noctisynth/semifold/commit/4774f04580338ebda64da61b7e6eb24bbdc67d6b): Check if Git repository is dirty or clean before versioning packages.

## v0.1.13

### Chores

- [`dccb0d2`](https://github.com/noctisynth/semifold/commit/dccb0d2312ea31e340a67ab2f6552a3918ce887a): Add readme and authors fields to `Cargo.toml`.

### New Features

- [`1ab8df9`](https://github.com/noctisynth/semifold/commit/1ab8df941408a707ec2ac0ca3c152257b8df7517): enhance dynamic version extraction for Python projects ([#29](https://github.com/noctisynth/semifold/pull/29) by @HsiangNianian)

## v0.1.12

### Bug Fixes

- [`ca8ad93`](https://github.com/noctisynth/semifold/commit/ca8ad93e48e2c87b5267d1769e5ae6b2f7d156d4): Assets should relative to repository root path instead of package root.

## v0.1.11

### New Features

- [`943a27c`](https://github.com/noctisynth/semifold/commit/943a27c26cfdb048b94f9c2e10ac12c6b3705392): Support upload GitHub release assets.

## v0.1.10

### New Features

- [`bbe6419`](https://github.com/noctisynth/semifold/commit/bbe6419bba673fc0e8a1ab7957d62fd0956b27ed): Skip publish private packages.

## v0.1.9

### New Features

- [`235d5f0`](https://github.com/noctisynth/semifold/commit/235d5f0e94b09094abb87caacd93bda46875121a): Support customize standard outputs for `stdout` and `stderr`.

## v0.1.8

### Bug Fixes

- [`6aa9bdf`](https://github.com/noctisynth/semifold/commit/6aa9bdfed57c03ca00bd39d4327409d8ac5087fc): Post version commands should run after all versioning tasks done.
- [`b95d9a5`](https://github.com/noctisynth/semifold/commit/b95d9a5714bb7bd0d4e66a688b0edeb51a34b812): Post version commands run for every package.

## v0.1.7

### New Features

- [`979e7de`](https://github.com/noctisynth/semifold/commit/979e7def35be9c1dd527822ab129f534eacec6ef): Support trigger post version commands after versioning.

## v0.1.6

### New Features

- [`450054a`](https://github.com/noctisynth/semifold/commit/450054ad8b496e1634553589d15815b0d8c8048a): add Python support to resolver ([#17](https://github.com/noctisynth/semifold/pull/17) by @HsiangNianian)

## v0.1.5

### New Features

- [`ee97bad`](https://github.com/noctisynth/semifold/commit/ee97bad45819d73f59f30d36ce0b50b1b4b61e78): Allow default publish fields in config.
- [`0171573`](https://github.com/noctisynth/semifold/commit/0171573c15463971538c85c801227145e4648e7d): Optimize empty config fields default serialization.

## v0.1.4

### New Features

- [`4bf1183`](https://github.com/noctisynth/semifold/commit/4bf11839b609bd6610423ede224fc89923fde079): Support Nodejs workspace resolve and version bumps.

## v0.1.3

### Bug Fixes

- [`66da4e2`](https://github.com/noctisynth/semifold/commit/66da4e2d6c26f8abe710f6a231b623127f3be090): Fix relative paths in packages sorting.

### New Features

- [`66da4e2`](https://github.com/noctisynth/semifold/commit/66da4e2d6c26f8abe710f6a231b623127f3be090): Support pre-check config before publishing packages.
- [`3a031ee`](https://github.com/noctisynth/semifold/commit/3a031ee7001923932f1ed6853bfd26e7fd431318): Embed semifold GitHub Actions workflow files.

## v0.1.2

### Bug Fixes

- [`5e1b994`](https://github.com/noctisynth/semifold/commit/5e1b994178fa662b630d700559cc888892b44813): Fix path of resolved package is relative path.

## v0.1.1

### Refactors

- [`2eb3d67`](https://github.com/noctisynth/semifold/commit/2eb3d67a373a55104562f2eaee7c6ebd33794510): Rewrite init command to support new configs.

### Bug Fixes

- [`2245ab9`](https://github.com/noctisynth/semifold/commit/2245ab96d869e5220d125f440747e035774a8c02): Fix packages release order.

### New Features

- [`d94df17`](https://github.com/noctisynth/semifold/commit/d94df1729f43bf6f159a00ed701e05e75aad2d02): Support create and apply changeset.
- [`1c06e8c`](https://github.com/noctisynth/semifold/commit/1c06e8cbe2f179fe0eb8a657249ba5573b1dfbaf): Use `toml_edit` to replace `toml`.
- [`9174302`](https://github.com/noctisynth/semifold/commit/9174302d76386cabb8de0948729b1e7267cc8e8f): Support `ci` and `status` command. ([#8](https://github.com/noctisynth/semifold/pull/8) by @fu050409)
- [`4007f78`](https://github.com/noctisynth/semifold/commit/4007f789aabf1aecaccb2066899b148edcd8c24b): Support `version` cli command.
- [`166ea37`](https://github.com/noctisynth/semifold/commit/166ea37e3cec9c690c0d23eec8c09067d8d9d38c): Auto generate changelog content while running version command.
