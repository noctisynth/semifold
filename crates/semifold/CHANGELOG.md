# Changelog

## v0.3.3

### Bug Fixes

- [`1177d67`](https://github.com/noctisynth/semifold/commit/1177d671fbdcb9c5b7be50ed987c480ee9508f77): Preserve and localize GitHub operation, API status, validation details, and underlying errors across CI pull requests, publishing releases and assets, status queries and comments, and changelog metadata lookup while retaining existing failure and recovery behavior.

## v0.3.2

### Bug Fixes

- [`1bda032`](https://github.com/noctisynth/semifold/commit/1bda03243247d168877cee92f449ea9ed9e800a4): Honor the global dry-run boundary for changeset creation, initialization, and pull-request status comments.
- [`c47ccd5`](https://github.com/noctisynth/semifold/commit/c47ccd5786ab38391efc8ca09cd3ab8cd4477960): Reject release branches that match the base branch during configuration loading, initialization, and rendered release execution before Semifold can force-update the base branch.
- [`c5ff0ce`](https://github.com/noctisynth/semifold/commit/c5ff0ce5d66d596e1b37d77cdd912ec4ed453ecc): Show structured operation, GitHub API status, message, documentation, and permission details when `status` cannot write its pull request comment, and document the repository workflow permissions required by generated GitHub Actions.
- [`1bda032`](https://github.com/noctisynth/semifold/commit/1bda03243247d168877cee92f449ea9ed9e800a4): Use shorter Chinese CLI sentences and avoid semicolons in localized output and help text.
- [`36b0180`](https://github.com/noctisynth/semifold/commit/36b018086f05c8c5cdd1ded6c591d3d6267ac22e): Make the global dry-run CLI end-to-end assertion follow the configured i18n translations across platforms.

## v0.3.1

### Bug Fixes

- [`d4773da`](https://github.com/noctisynth/semifold/commit/d4773da93ff46737a771485f9a097f714bbc3f19): Restore HTTPS transport support for pushing the generated release branch after the git2 dependency upgrade.

### Chores

- [`05a2e7c`](https://github.com/noctisynth/semifold/commit/05a2e7c6b85697cbd091f74698d88a87f8678f01): Upgrade Rust and JavaScript dependencies to their latest compatible versions, including the required API migrations for Git, HTTP, hashing, localization, MCP, and configuration handling.

### New Features

- [`3454415`](https://github.com/noctisynth/semifold/commit/34544159c278e1c9096e1e002ab6966f978300e6): Allow release commit messages and pull request titles to use optional strict release context templates while preserving the existing defaults.
- [`1b2317a`](https://github.com/noctisynth/semifold/commit/1b2317add403852d475ba95c4a0c0ebe438cdcd8): Publish Semifold as an npm-installed Node.js CLI backed by napi-rs generated native packages for supported macOS, Linux glibc, and Windows targets.

## v0.3.0

### New Features

- [`c64a001`](https://github.com/noctisynth/semifold/commit/c64a0011732bb40044d9486ef708fb03bfc9c650): Add an optional package-level `publish` override that defaults to manifest or plugin discovery and explicitly controls registry pre-check and publish command eligibility.
- [`61697f3`](https://github.com/noctisynth/semifold/commit/61697f3ffb25614e8633b041cfd1b96effa6632c): Make `smif init` assign ecosystem-prefixed IDs to packages that share a manifest name across ecosystems, with deterministic numeric suffixes when needed.
- [`d8d7bc1`](https://github.com/noctisynth/semifold/commit/d8d7bc110712743ab35b83fa861ddcc1a66812d1): Suggest `smif config migrate` when a TOML configuration cannot be loaded under the current contract, while keeping unrelated project and format errors focused on their actual cause.

## v0.3.0-rc.6

### Bug Fixes

- [`b17acdb`](https://github.com/noctisynth/semifold/commit/b17acdb420cd9d02e6f85716219f6eba3dba899a): Run config migrate against raw TOML before strict project loading, and warn when channel updates target Node.js packages whose npm publish command lacks an explicit dist-tag.
- [`92cf6d7`](https://github.com/noctisynth/semifold/commit/92cf6d70e7f516965461cbd5801b5b1640d27f80): Continue a planned GitHub Release after registry preflight finds an existing package version, while keeping registry commands skipped, reporting both outcomes, and leaving existing Release assets untouched.

### New Features

- [`ca56371`](https://github.com/noctisynth/semifold/commit/ca56371c162e7845e8f8652d9e2b488021a85de8): Configure repository-local dynamic ecosystem plugins by stable ID with optional SHA-256 pins and exact HTTPS origins, and route discovery, workspace loading, config sync, and version edit planning through their authenticated adapters.
- [`90f27bb`](https://github.com/noctisynth/semifold/commit/90f27bb8bd85a7d0a1dbb6830cfec2ed46ec1152): Replace the MCP changeset surface with lazily loaded, structured get/create/update/delete tools, optimistic SHA-256 revisions, dry-run planning, localized errors, and panic isolation that keeps the stdio server available after failed calls.

## v0.3.0-rc.5

### Bug Fixes

- [`f123236`](https://github.com/noctisynth/semifold/commit/f12323663c6cd9bbfb995c5fad487f662aba0993): Render the release plan commit SHA as bare text in GitHub status comments so GitHub automatically displays it as a shortened, clickable commit link.
- [`1e57078`](https://github.com/noctisynth/semifold/commit/1e57078c93fc87d7efa0eccc532bb56fa457d9b0): Make `smif ci` reuse the version and publish execution paths so GitHub Actions receives the same versioned outputs, and expose them through stable step and job output mappings in generated workflows.
- [`c07709d`](https://github.com/noctisynth/semifold/commit/c07709d025f411ec289620a7fdb5e9f4b3b81585): Rename the `init` workflow selection flags to `--github-actions` and `--no-github-actions` so their purpose is explicit in scripts and limited-interaction environments.

## v0.3.0-rc.4

### Bug Fixes

- [`4e90704`](https://github.com/noctisynth/semifold/commit/4e90704f98c86358d201b473e64d7ebddaa610c7): Use `-m` as the short form of `smif commit --summary`, matching familiar commit message conventions.

### New Features

- [`d080a47`](https://github.com/noctisynth/semifold/commit/d080a4794c27a56bec09325762cf85cfb3d36608): Make every init and changeset creation prompt optional through equivalent CLI arguments so the same commands run predictably in CI/CD and constrained agent environments with stdin closed. Missing arguments now fail fast with actionable guidance when prompting is unavailable.
- [`f52501d`](https://github.com/noctisynth/semifold/commit/f52501d22f5ed4bd1408111072a54472880ea63b): Allow `smif commit -m` to be repeated, treating each value as an ordered summary paragraph that the default changelog template renders as a continuation paragraph within the same changeset list item.

## v0.3.0-rc.3

### Bug Fixes

- [`ee07213`](https://github.com/noctisynth/semifold/commit/ee07213d03b5d9ce10c0e4927f721367657779a9): Add semantic colors to package, version, status, and detail cells in the publish result table while preserving wide-character alignment and plain-text fallback behavior.
- [`b34131e`](https://github.com/noctisynth/semifold/commit/b34131e08c6e6db0d3562d6d554847587c90953e): Replace the verbose numbered post-version command queue with a compact, ordered, deduplicated list of affected packages while retaining per-command execution feedback.
- [`23ac210`](https://github.com/noctisynth/semifold/commit/23ac210a0f0e2511d2cef079cbc2678f2ff80cac): Detect changesets introduced or changed by the current pull request through the paginated GitHub files API, list them in status comments, and base the explanatory empty state on that branch scope.
- [`1467a2b`](https://github.com/noctisynth/semifold/commit/1467a2b34f50add6c62f2133c8b4f4af8e243df9): Align CLI fact columns by Unicode display width and suspend dynamic progress while post-version commands inherit the terminal, preventing spinner redraws from overwriting child-process output.
- [`d37a03e`](https://github.com/noctisynth/semifold/commit/d37a03e97f10ddbc5989670250b54473449c9f21): Improve GitHub pull request release-plan comments with an always-visible version table, reasons, safe comment ownership markers, and an explanatory empty state for post-merge publishing.
- [`9e042d4`](https://github.com/noctisynth/semifold/commit/9e042d469769d6f0a74537a359c417028ded2330): Normalize source-wrapped lines within each changeset paragraph to spaces in the default changelog template while preserving explicit blank-line paragraph boundaries and custom template access to the original summary structure.
- [`b31f342`](https://github.com/noctisynth/semifold/commit/b31f342ef8421d6a312b83099b20d31710ae10ca): Report post-version commands in their actual sequential execution order. Commands with captured output show per-command progress, while commands inheriting the terminal only print a result after their child process exits.
- [`ca614dd`](https://github.com/noctisynth/semifold/commit/ca614dd6c061a8a0554c035ad73037557c0a8f0c): List the complete ordered package and command queue before executing post-version hooks, while retaining per-command progress and completion feedback during execution.

### New Features

- [`b52d510`](https://github.com/noctisynth/semifold/commit/b52d510607c00954cb647f7d3e8b70ee7095f1f5): Expose versioned, allowlisted GitHub Actions outputs from `smif version` and `smif publish`. Version outputs preserve the release plan fingerprint, branch, and package versions, while publish outputs retain complete package recovery states after partial failures without leaking command or environment configuration.
- [`7cc9b8b`](https://github.com/noctisynth/semifold/commit/7cc9b8b05f8ab9b331f61f45c5a15776f3b162db): Introduce a unified terminal presentation layer with indicatif-powered TTY progress, stable CI and redirected output, Unicode-aware tables, explicit dry-run banners, command summaries, and actionable partial-publish recovery feedback. Debug output no longer dumps complete configuration or GitHub event payloads.

## v0.3.0-rc.2

### New Features

- [`c90d417`](https://github.com/noctisynth/semifold/commit/c90d4173d28e04e951183b3f10bc28905df0df2a): Make HTTP publish pre-checks inject an overridable runtime User-Agent, retry transient failures
    using configured delays and Retry-After, and report bounded response details for registry errors.
- [`7ca5d48`](https://github.com/noctisynth/semifold/commit/7ca5d48f26c5de2f888d89c4b45d8650165b9b74): Limit Semifold configuration to TOML and add typed HTTP or command publish pre-checks. Command
    pre-checks exchange package metadata and existence results through a strict JSON Lines protocol,
    while HTTP checks now fail safely on statuses other than 200 and 404.

## v0.3.0-rc.1

### Bug Fixes

- [`b4871b3`](https://github.com/noctisynth/semifold/commit/b4871b3749d814ddb9b7ca38e84c0e2f46d78e37): Stop forcing Cargo offline mode in the Rust resolver's default post-version lockfile generation command.

### New Features

- [`2f843f3`](https://github.com/noctisynth/semifold/commit/2f843f38b196bf61cffd41daa6ae07cdccd6fe94): Allow workspace owners to customize complete changelog release blocks and individual changeset entries with strict MiniJinja templates. Template contexts now expose structured summaries and commit metadata, while stable release markers let publish consume arbitrary rendered formats without breaking legacy changelogs.
- [`42f88c9`](https://github.com/noctisynth/semifold/commit/42f88c9302f8d974c8351aaa30297b0aff2fb002): Write the built-in release and changeset changelog templates into newly initialized configuration files so users can discover and customize them immediately, while retaining the same templates as fallbacks for older configurations.
- [`145ccec`](https://github.com/noctisynth/semifold/commit/145ccecb196b51bbe40d1123c6b7ad6f4678aa25): Add an optional per-package `github-release` policy. Public packages keep GitHub Releases enabled by
    default, private packages keep them disabled by default, and either default can now be overridden
    explicitly without changing registry publishability.

## v0.3.0-rc.0

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

- [`c711072`](https://github.com/noctisynth/semifold/commit/c711072eb5c1a67c164dd13cc6e78b4ab09bd26e): Build complete publish plans with project, changelog, and Forge release facts before execution.

    CLI and CI now share the same publish service, while CLI and MCP use one validated changeset
    creation service instead of duplicating resolver and filesystem operations.

- [`a05f89a`](https://github.com/noctisynth/semifold/commit/a05f89a5717b2932d850277d497b036533439036): Add immutable plans and application-service entrypoints for initialization, configuration
    migration, release-channel updates, and worktree validation.

    Keep CLI modules focused on argument parsing, interaction, embedded asset loading, and localized
    result rendering while package discovery, configuration construction, validation, and writes are
    owned by the engine.


### Dependencies

- Update semifold-engine to 0.2.0-rc.0.

## v0.3.0-beta.4

### Bug Fixes

- [`7a8b7d0`](https://github.com/noctisynth/semifold/commit/7a8b7d0feb9c59b2ad56fd93a206b32310c4ea1b): Resolve release asset paths only after package publish commands finish, so generated artifacts are
    included. Packages without a changelog are now skipped before registry checks or publish commands.

- [`60fbc32`](https://github.com/noctisynth/semifold/commit/60fbc32b1ea91cac50476b1682bbe2df755bcb19): Update publish reports and channel configuration through safe iteration and lookup instead of
    potentially panicking collection indexing.


### Changes

- [`ea86a6a`](https://github.com/noctisynth/semifold/commit/ea86a6a87bb6b155d13e1aceabe9cf7f3da974fd): Isolate config editor tests when the suite runs concurrently.

    Temporary configuration paths now include the process ID and an atomic sequence so parallel tests
    cannot delete one another's fixtures.


### New Features

- [`7a8b7d0`](https://github.com/noctisynth/semifold/commit/7a8b7d0feb9c59b2ad56fd93a206b32310c4ea1b): Migrate known snake-case configuration fields to kebab-case with conflict detection while keeping
    the normal configuration loader strict.

- [`2624d2d`](https://github.com/noctisynth/semifold/commit/2624d2d12fb9678d3f622a2aac69acddbd3af5f4): Plan package publishing from the current workspace with package-scoped template contexts.

    Registry checks and publish commands now use strict package variables without depending on a
    persisted release context or consumed changesets.

- [`ea86a6a`](https://github.com/noctisynth/semifold/commit/ea86a6a87bb6b155d13e1aceabe9cf7f3da974fd): Support Rust packages that inherit `workspace.package.version`.

    Shared version sources now merge bumps across every inheriting crate, validate channel policy, keep
    private crates in the version closure, and update the owning workspace manifest exactly once.

- [`ea86a6a`](https://github.com/noctisynth/semifold/commit/ea86a6a87bb6b155d13e1aceabe9cf7f3da974fd): Execute publish plans through injectable external capability ports.

    Registry preflights now complete before commands start, command dry-run permissions are reported
    explicitly, GitHub releases and assets use a Forge adapter, and partial failures return structured
    package statuses with retry guidance.

- [`2624d2d`](https://github.com/noctisynth/semifold/commit/2624d2d12fb9678d3f622a2aac69acddbd3af5f4): Add deterministic workspace release contexts and strict release branch templates.

    Release branch templates now consume a workspace release view derived from the same validated plan
    that versioning applies.


### Refactors

- [`ea86a6a`](https://github.com/noctisynth/semifold/commit/ea86a6a87bb6b155d13e1aceabe9cf7f3da974fd): Use kebab-case for every Semifold configuration field.

    Snake-case configuration keys are no longer supported. Repository configuration, generated
    configuration, and fixtures now use fields such as `dry-run`, `extra-env`, and `extra-headers`.

- [`2624d2d`](https://github.com/noctisynth/semifold/commit/2624d2d12fb9678d3f622a2aac69acddbd3af5f4): Model changelog rendering as immutable package and changeset facts.

    Changelog collection now resolves package sections and optional commit and pull request metadata
    before passing a capability-free aggregate context to the Markdown formatter.

- [`ea86a6a`](https://github.com/noctisynth/semifold/commit/ea86a6a87bb6b155d13e1aceabe9cf7f3da974fd): Build release pull requests from an explicit workspace release context.

    Release pull request rendering now preserves the existing title and body format while ordering
    package changelogs deterministically and avoiding any implicit primary package.

## v0.3.0-beta.3

### Bug Fixes

- [`de16b01`](https://github.com/noctisynth/semifold/commit/de16b017c5163a563aee17f6dfc2fd37345ca74a): Render multiline changeset entries as valid nested Markdown paragraphs

    Blank lines in a changeset summary now separate content without producing whitespace-only
    paragraphs. Every non-empty continuation line is emitted after a blank line with four spaces of
    indentation, and regression coverage uses the same front matter and prose shape as real changesets.

- [`9da27d5`](https://github.com/noctisynth/semifold/commit/9da27d5191fe15900de27882e6a6cac0d8061e1c): Preserve wrapped lines within changelog paragraphs

    Changelog formatting now uses blank lines, rather than every physical line break, as paragraph
    boundaries. Hard-wrapped lines remain together inside an indented continuation paragraph.


### New Features

- [`e8f8a09`](https://github.com/noctisynth/semifold/commit/e8f8a0966142aacdab13a33d41332fd9612e4cf9): Add one-shot channel transition bump overrides to `config channel set`, including a preserve mode for entering prerelease channels without raising the stable version base.

## v0.3.0-beta.2

### Bug Fixes

- [`90e6cd6`](https://github.com/noctisynth/semifold/commit/90e6cd697eed4686dcfc946eb833ba76bc89c1ff): Render multiline changeset summaries as separate paragraphs in one changelog list item.

## v0.3.0-beta.1

### Bug Fixes

- [`fd19bf0`](https://github.com/noctisynth/semifold/commit/fd19bf09d523d07f75813c064b97b8ca899d2d64): Preserve configured package IDs when manifests share names across ecosystems.

### New Features

- [`e015e6d`](https://github.com/noctisynth/semifold/commit/e015e6d234f97b87e7422431b4b3e659a3a218df): Validate ecosystem release channels and encode Python PEP 440 pre- and post-release versions.
- [`e108ed0`](https://github.com/noctisynth/semifold/commit/e108ed0721d928d8ca543ce7bc3c00db0030afe3): Support explicit cross-ecosystem dependency ordering and release propagation.

### Refactors

- [`0dab79f`](https://github.com/noctisynth/semifold/commit/0dab79fe5ac1ba645820c831e4605da4b1aa7a1f): Move publish command execution and dry-run handling out of ecosystem resolvers.
- [`1932935`](https://github.com/noctisynth/semifold/commit/1932935da7bd936f07c4f3bc58b01e9552350994): Document and enforce dependency-kind ordering and propagation policies.
- [`f74c91a`](https://github.com/noctisynth/semifold/commit/f74c91a25ae2fe97f4657a77f3ac1a5f1156a2f8): Converge ecosystem discovery, inspection, publishing, and fixture coverage on the EcosystemAdapter interface.

## v0.3.0-beta.0

### Refactors

- [`0d9f35d`](https://github.com/noctisynth/semifold/commit/0d9f35d368a5b3a84ce93ba20c9dc4dcf96a5090): Route Rust package discovery, inspection, and release edit planning through the side-effect-free ecosystem adapter boundary.
- [`a88431b`](https://github.com/noctisynth/semifold/commit/a88431bcca5a4fa60b05b0fd291ec3781f7b906d): Route C++ package discovery, inspection, and CMake/vcpkg edit planning through the ecosystem adapter with recursive in-root workspace discovery.
- [`31a668f`](https://github.com/noctisynth/semifold/commit/31a668f13b31501f406f2c08f32f012780537231): Route Python package discovery, inspection, and native version file planning through the ecosystem adapter while preserving read-only Cargo version sources.
- [`9e6b717`](https://github.com/noctisynth/semifold/commit/9e6b7170b2b75f44d26b6d5aa80917daa63149ff): Route Node.js package discovery, inspection, and release edit planning through the ecosystem adapter boundary with configured package id support.

## v0.3.0-alpha.9

### Bug Fixes

- [`6dabc7d`](https://github.com/noctisynth/semifold/commit/6dabc7d6dce7c4f69048f7cc86030621f7c1ba6c): Clean the current and remaining temporary files when a release file replacement fails.
- [`fce12df`](https://github.com/noctisynth/semifold/commit/fce12dff6bf2282af11f9129d3620ef852daf1e8): Plan Rust package and workspace dependency version edits as one deterministic batch.

### New Features

- [`04e0a5e`](https://github.com/noctisynth/semifold/commit/04e0a5e5da84a3e5703ba9eaa839a54b17f178ad): Plan Python manifest and source version edits through the unified release plan without modifying Cargo.toml.

### Refactors

- [`4f10fcf`](https://github.com/noctisynth/semifold/commit/4f10fcf62ed28085c1bd9a2d6db5f4578895b742): Separate read-only release edit validation from file application and strengthen zero-write failure coverage.

## v0.3.0-alpha.8

### Bug Fixes

- [`3ef7c05`](https://github.com/noctisynth/semifold/commit/3ef7c0538172b0e3fd4e3755d44dfe5c26255e40): Warn when changelog generation continues without pull request metadata.
- [`47b85dc`](https://github.com/noctisynth/semifold/commit/47b85dc4e83fa33f0981efcc7dc12b11fb7ddae8): Publish packages in the deterministic WorkspaceGraph topological order so registry dependencies are available before their dependents.

### New Features

- [`419d267`](https://github.com/noctisynth/semifold/commit/419d267f7f15a2ff930f5fc9f8cd256735b8b07a): Plan CMake and vcpkg version edits before applying C++ release changes.

### Refactors

- [`472c745`](https://github.com/noctisynth/semifold/commit/472c745c771e29a9bc1c741cdbdd8d8e70a259cb): Prepare release planning for ecosystem adapters that produce multiple file edits per package.

## v0.3.0-alpha.7

### Refactors

- [`f42b195`](https://github.com/noctisynth/semifold/commit/f42b195058a4f6972f1a7468023c0512f4845d24): Keep recoverable input failures as errors while using documented `expect` calls for verified internal invariants.
- [`56e0686`](https://github.com/noctisynth/semifold/commit/56e06863fb2846497ed0b79417d68f3cf17eb8ca): Enforce the production unwrap policy through shared workspace Clippy configuration while allowing documented internal expects.

### Bug Fixes

- [`39a5a2a`](https://github.com/noctisynth/semifold/commit/39a5a2a6aa8e0a965eeec62b769816d329b2b221): Keep applied files and changesets after a post-version failure, with structured recovery details.
- [`3b961e0`](https://github.com/noctisynth/semifold/commit/3b961e069ea4bd5b43e5b29bf2f5c5fc39414c9b): Return errors instead of panicking when release planning, configuration, changelog, and resolver invariants are unavailable.

### New Features

- [`efa7e5b`](https://github.com/noctisynth/semifold/commit/efa7e5be09ca8268155a70391ab4a189c603fc9f): Validate changelog edits as part of the immutable release plan before applying version changes, without remote metadata requests during dry runs.
- [`09e7af6`](https://github.com/noctisynth/semifold/commit/09e7af6e76bed7a01b72e6a675504ab308732d2f): Plan Rust and Node changelog updates as validated file edits that can safely create a missing changelog.
- [`2014b6e`](https://github.com/noctisynth/semifold/commit/2014b6eb2139643e130b51b4ad7f0223f0e826a0): Return a structured report for completed planned version file edits.

## v0.3.0-alpha.6

### Bug Fixes

- [`1c90682`](https://github.com/noctisynth/semifold/commit/1c90682632e694b914d5ba1aefd7118344115c81): Make versioning consume the shared release plan so mixed Rust and Node.js workspaces do not panic when a Node.js package has a changeset.
- [`5d67c50`](https://github.com/noctisynth/semifold/commit/5d67c509d0f203b4ce50d48cf392d2d805304170): Complete the Chinese CLI locale and verify it has the same translation keys as English without relying on fallback text.

### New Features

- [`93e5c74`](https://github.com/noctisynth/semifold/commit/93e5c746f337bef0e0729bcb71b4537c60a62b73): Add a validated FileEdit executor that rejects changed or conflicting targets before writing and applies replacements atomically.
- [`44ee660`](https://github.com/noctisynth/semifold/commit/44ee660cf4a0993bcb87cea2e43e9f391aa7119d): Plan and atomically apply Rust and Node manifest version edits through the shared release plan.

## v0.3.0-alpha.5

### Bug Fixes

- [`e0f036f`](https://github.com/noctisynth/semifold/commit/e0f036fc681358fda2b06ec13a2d7fd379d3935d): Return a typed unsupported-format error when config sync is invoked for JSON configuration files.
- [`64a2fbb`](https://github.com/noctisynth/semifold/commit/64a2fbb6d913edcb59e49435212da190a1307d04): Clarify that `init --force` is only for explicit reinitialization and direct workspace package changes to `config sync`.
- [`48697f8`](https://github.com/noctisynth/semifold/commit/48697f8069b7f3c623f563e466c8896fd40476eb): Insert newly discovered package tables in deterministic package-ID order without reordering existing configuration.

### New Features

- [`563a8b9`](https://github.com/noctisynth/semifold/commit/563a8b966ddea828ba84a4444f7c42ca268abf5d): Add `smif config sync --prune` to atomically remove package configuration entries that are absent from a complete workspace scan.
- [`c591e35`](https://github.com/noctisynth/semifold/commit/c591e3563a81f3dcae7d0c62ed294ed88058fbe6): Add repeatable resolver scoping to `smif config sync` and prevent pruning from partial scans.
- [`27adfcf`](https://github.com/noctisynth/semifold/commit/27adfcf9e70dd04f08cb928455cf624882637b0c): Add `smif config sync --check` for CI-safe configuration drift detection without file writes.
- [`55496de`](https://github.com/noctisynth/semifold/commit/55496ded28bbd1c133ba5d3af795a0a1ae4623f1): Add the `smif config sync` command to safely synchronize discovered packages into TOML configuration, with dry-run plan output and preserved formatting.

## v0.3.0-alpha.4

### New Features

- [`8377fd7`](https://github.com/noctisynth/semifold/commit/8377fd77c6f33fe834b601675e301f22510d44d9): Add a TOML config editor that validates and applies package sync plans while preserving release configuration, comments, and manual package fields.
- [`a9a0ac9`](https://github.com/noctisynth/semifold/commit/a9a0ac9c59b99eddb2ee0694b9629d0a115658c8): Bridge configured packages, shared discovery, and pending changesets into config sync planning, including warnings for changesets that reference renamed packages.
- [`a8fb0f2`](https://github.com/noctisynth/semifold/commit/a8fb0f22fb64739091406805ad290e09d674973e): Add shared safe package path normalization for workspace discovery and config synchronization.
- [`3abdf29`](https://github.com/noctisynth/semifold/commit/3abdf29df5cbbe5542ff72a1f98c276dd74e4406): Add shared deterministic package discovery for init and config synchronization, and fail incomplete workspace scans instead of silently skipping invalid packages.

## v0.3.0-alpha.3

### New Features

- [`6bd8cac`](https://github.com/noctisynth/semifold/commit/6bd8cac90a389b81aa9eb5ef3a1b7e38fbe44cfb): Render status and CI pull request comments from the immutable core release plan.

## v0.3.0-alpha.2

### New Features

- [`5bea426`](https://github.com/noctisynth/semifold/commit/5bea42624aa7ba73034ad0fba8dc1c9c14da7419): Bridge configured Rust, Node.js, Python, and C++ packages into the new cross-ecosystem workspace graph.

## v0.3.0-alpha.1

### Chores

- [`38d09ea`](https://github.com/noctisynth/semifold/commit/38d09ea43b7f7942307b3caabdae6a71e4279fcd): Add end-to-end CLI regression coverage for release planning and config updates.

### Bug Fixes

- [`b1bbd07`](https://github.com/noctisynth/semifold/commit/b1bbd0770cff5e1c998704761f34eabff0e7c997): Keep CLI end-to-end tests isolated from host GitHub Actions environment variables.
- [`945657b`](https://github.com/noctisynth/semifold/commit/945657baa2b2dcae3bad60f59cba2d1ab1d66568): Only release Rust runtime dependents when their internal dependency version constraints are no longer satisfied.

## v0.3.0-alpha.0

### Bug Fixes

- [`1f2498c`](https://github.com/noctisynth/semifold/commit/1f2498cf34d99663199d49a1ab9bbc7d88a34c1c): Localize configuration command help, status output, and error messages.
- [`334977f`](https://github.com/noctisynth/semifold/commit/334977ff31af0b0a0858a82c1e9c383e5f333069): Automatically include transitive Rust dependents in version releases and rewrite their internal dependency versions before post-version commands run.
- [`8a9a56e`](https://github.com/noctisynth/semifold/commit/8a9a56ea402c1e4364c2ed210b3696a3d0b37f73): Fix formatting lint violations reported by Rust 1.97.

### New Features

- [`7b59b5d`](https://github.com/noctisynth/semifold/commit/7b59b5dea42b8e4feeb9c3d3887e06c0c8ce950d): Generate Dependencies changelog entries for packages automatically released because an internal dependency changed.
- [`1f2498c`](https://github.com/noctisynth/semifold/commit/1f2498cf34d99663199d49a1ab9bbc7d88a34c1c): Add alpha release-channel lifecycle support, including stable-base selection, in-channel sequencing, and channel switching.
- [`1f2498c`](https://github.com/noctisynth/semifold/commit/1f2498cf34d99663199d49a1ab9bbc7d88a34c1c): Add commands to set, clear, check, and bulk-manage configured package release channels.
- [`1f2498c`](https://github.com/noctisynth/semifold/commit/1f2498cf34d99663199d49a1ab9bbc7d88a34c1c): Add TOML configuration migration from legacy version modes to release channels.

## v0.2.16

### New Features

- [`1350979`](https://github.com/noctisynth/semifold/commit/1350979c131c09552cde780ad109fc2ede217bca): Add i18n support to MCP server with localized error messages and descriptions for schemars
- [`564a7b3`](https://github.com/noctisynth/semifold/commit/564a7b37233308685a99dabd5e9c006fc9b1f213): Add mcp server ([#79](https://github.com/noctisynth/semifold/pull/79) by @YESWmeshade)

## v0.2.15

### Bug Fixes

- [`64041fa`](https://github.com/noctisynth/semifold/commit/64041fa8b241297a200d2dfc97dea0dc899346b2): Fix status CI workflow file hardcoded main branch. ([#76](https://github.com/noctisynth/semifold/pull/76) by @fu050409)

## v0.2.14

### New Features

- [`2814bed`](https://github.com/noctisynth/semifold/commit/2814bedcd13d3b35490628becea25a63795b082d): Detect if GitHub release tag exists.

### Bug Fixes

- [`65a53a7`](https://github.com/noctisynth/semifold/commit/65a53a7a5e121f0fa52e258f14681aa727a473c9): Auto add version field for path based dependencies.

## v0.2.13

### Bug Fixes

- [`1eb7732`](https://github.com/noctisynth/semifold/commit/1eb7732b230b9c809e292f6ec3324e3eb7dfba34): Ensure all assets filtered by glob patterns are files.

## v0.2.12

### Bug Fixes

- [`df6e2ab`](https://github.com/noctisynth/semifold/commit/df6e2abd48beff959570d9cce997a7a00c829ee9): Always resolve asset files and use full path glob pattern instead.

## v0.2.11

### New Features

- [`fd41853`](https://github.com/noctisynth/semifold/commit/fd41853260fbb5b1e61a41373c24684d2a38e22e): Support search upload assets by glob pattern.

## v0.2.10

### Bug Fixes

- [`306d737`](https://github.com/noctisynth/semifold/commit/306d7375fb2da7adabf9ad4b268e119674732c17): Fix resolver display names, use camel case instead.

## v0.2.9

### New Features

- [`d8959d0`](https://github.com/noctisynth/semifold/commit/d8959d02b980e2407fa95009e8afbf4c4375b1c0): Add cli.ci section with 7 translation keys to en.toml and zh.toml and replace hardcoded English strings in ci.rs with t!() macro calls ([#58](https://github.com/noctisynth/semifold/pull/58) by @BegoniaHe)

### Bug Fixes

- [`df11377`](https://github.com/noctisynth/semifold/commit/df11377f2143106eab2cd0c8654fe76624502892): Fix panic if `.github/workflows` does not exists.

### Refactors

- [`d8959d0`](https://github.com/noctisynth/semifold/commit/d8959d02b980e2407fa95009e8afbf4c4375b1c0): 1. Add optional clap dependency to semifold-resolver 2. Conditionally derive ValueEnum on ResolverType 3. Remove duplicate ResolverType definition in init.rs ([#58](https://github.com/noctisynth/semifold/pull/58) by @BegoniaHe)

## v0.2.8

### New Features

- [`4bee630`](https://github.com/noctisynth/semifold/commit/4bee630d87e143e22ca954d2c10634936d1879e2): Optimize CI template files for auto detect whether should add CLI token envs.
- [`5ff36f0`](https://github.com/noctisynth/semifold/commit/5ff36f02db26132a3ec01089d46069a70210324d): Defaults to select all packages if only a package is found in a single workspace.

## v0.2.7

### New Features

- [`8d6ec13`](https://github.com/noctisynth/semifold/commit/8d6ec138b31560337dbfdc854e61d68072ca9be1): add `cpp` option in `init` command, which was forgotten in the previous commits ([#54](https://github.com/noctisynth/semifold/pull/54) by @BegoniaHe)

## v0.2.6

### New Features

- [`d39c94a`](https://github.com/noctisynth/semifold/commit/d39c94a3b30df9640fb147c77a820a87c9167319): bump version for semifold cpp support ([#52](https://github.com/noctisynth/semifold/pull/52) by @BegoniaHe)

### Bug Fixes

- [`4d19256`](https://github.com/noctisynth/semifold/commit/4d1925639592cd2d9149a2f865ceb38c935e47a7): Nothing will be reported if a pull request submitted from a main branch of a forked repository.
- [`4d19256`](https://github.com/noctisynth/semifold/commit/4d1925639592cd2d9149a2f865ceb38c935e47a7): Fix invalid changeset content with empty tag.

## v0.2.5

### Chores

- [`6f381fa`](https://github.com/noctisynth/semifold/commit/6f381fa674e1fbc9bc5ed12124e1e64047de6b7c): Rename all artifacts to `artifact_name` in CI to ensure the filename of all downloaded.

## v0.2.4

### New Features

- [`549d339`](https://github.com/noctisynth/semifold/commit/549d33903c8731f334305fc1d57f3291f1437f02): Optimize CI template and add shell install scripts.

## v0.2.3

### Bug Fixes

- [`2505304`](https://github.com/noctisynth/semifold/commit/2505304e487bd00bf1c2c87e2c26909b677de202): Ignore errors reported from IO due to invalid asset file which is a dir.

## v0.2.2

### Bug Fixes

- [`89e5ddd`](https://github.com/noctisynth/semifold/commit/89e5dddf3059dcd69579f431d21dbbf5742c56d8): Skip 422 issue reported from GitHub when trying to create release.

### Chores

- [`bc7de21`](https://github.com/noctisynth/semifold/commit/bc7de21ba72b63b2c558e7d1906517c2301cb153): Release cross-platform artifacts to GitHub Release.

## v0.2.1

### New Features

- [`1996e48`](https://github.com/noctisynth/semifold/commit/1996e485d9b61e837c660d8b5683b6d11cc6f863): Default to create GitHub releases for private packages.

## v0.2.0

### Refactors

- [`86c97d9`](https://github.com/noctisynth/semifold/commit/86c97d9a63cff0931588c434908bcf4fe91f7805): Mark `--dry-run` flag as global options.

### Bug Fixes

- [`cf4c08b`](https://github.com/noctisynth/semifold/commit/cf4c08b72e6003062a18bd077db3f92ea98a86cd): Fix panic if running publish cli on local machine.
- [`c35a34e`](https://github.com/noctisynth/semifold/commit/c35a34e5a3854cc949fac3d2ee9a80778cc0fd12): Add `id-token` permission for GitHub Actions workflow files to support Node.js publish.
- [`5bc444a`](https://github.com/noctisynth/semifold/commit/5bc444a4e4ec5b864d63cef23687aad52cc854d7): Fix Nodejs default publish command.

### New Features

- [`e009c7e`](https://github.com/noctisynth/semifold/commit/e009c7ec0d2908cdf6bf11430a7c0db46f8f40ad): Support running commands in dry run mode.
- [`b346aa7`](https://github.com/noctisynth/semifold/commit/b346aa74585fbe4196d303ff5b34934d6b8493b5): Prevent publish process with dirty git working tree.
- [`98e4a7d`](https://github.com/noctisynth/semifold/commit/98e4a7d7ba33a1179fd542fdef0c7a4011ecab64): Sort packages and cache version bumps in version process, fix Rust workspace related packages version bump.
- [`27b53b2`](https://github.com/noctisynth/semifold/commit/27b53b28c15e7056f54e0f61ae8f688cf714e59a): When switching from pre-release mode to production mode, ignore minor and major version bumps and remove only the pre-release tag.
- [`985a9f5`](https://github.com/noctisynth/semifold/commit/985a9f5f7614877d8abf54404112481fa45f4a75): Enhance i18n supports for Semifold CLI

## v0.1.19

### Bug Fixes

- [`1862ba8`](https://github.com/noctisynth/semifold/commit/1862ba8d7df701893a65b9187cdbaf9ecaf20fa0): Fix version bump when version mode changed from pre-release to semantic.

## v0.1.18

### New Features

- [`4856c7d`](https://github.com/noctisynth/semifold/commit/4856c7d14bb2bd3622f9ae29f8b75e5ad2f60165): Improve compatibility to `changesets` and `covector`, allow empty tag key now.
- [`ff9d9a1`](https://github.com/noctisynth/semifold/commit/ff9d9a150e5a968cd4f1d1ab7dcdfb29780e0e35): Block the publish process before pre-checking private packages.

## v0.1.17

### Performance Improvements

- [`940c9fc`](https://github.com/noctisynth/semifold/commit/940c9fcfb0422fd98e239401b01683945011227e): Disable useless features and create release profile for binary size optimizations.

## v0.1.16

### New Features

- [`35dad5f`](https://github.com/noctisynth/semifold/commit/35dad5f2d1b5348b2740cd4269005f52b5ca599b): Support pre-release versioning mode.

### Bug Fixes

- [`9d625f4`](https://github.com/noctisynth/semifold/commit/9d625f4309fe19c60e380a6c64348fe2a83feb48): Fix version parse from changelog startswith `v`.

### Refactors

- [`3f27105`](https://github.com/noctisynth/semifold/commit/3f27105467f33cfcb03b6f62a72f9c912ec8827b): Refactor `semifold` crate to support library mode.

## v0.1.15

### Bug Fixes

- [`c791e93`](https://github.com/noctisynth/semifold/commit/c791e9320694354c34aac2e1f2ad0ec4b596ee1a): Fix remaining issues of project renaming.
- [`3a031ee`](https://github.com/noctisynth/semifold/commit/3a031ee7001923932f1ed6853bfd26e7fd431318): Fix delivered template GitHub Actions workflow files.

### New Features

- [`1e55e71`](https://github.com/noctisynth/semifold/commit/1e55e7132c7e7bc1ef375a15c273405845e404be): Select all packages by default if variant is `patch`.
- [`4774f04`](https://github.com/noctisynth/semifold/commit/4774f04580338ebda64da61b7e6eb24bbdc67d6b): Check if Git repository is dirty or clean before versioning packages.

## v0.1.14

### Chores

- [`dccb0d2`](https://github.com/noctisynth/semifold/commit/dccb0d2312ea31e340a67ab2f6552a3918ce887a): Add readme and authors fields to `Cargo.toml`.

## v0.1.13

### Bug Fixes

- [`ca8ad93`](https://github.com/noctisynth/semifold/commit/ca8ad93e48e2c87b5267d1769e5ae6b2f7d156d4): Assets should relative to repository root path instead of package root.

## v0.1.12

### New Features

- [`943a27c`](https://github.com/noctisynth/semifold/commit/943a27c26cfdb048b94f9c2e10ac12c6b3705392): Support upload GitHub release assets.

## v0.1.11

### New Features

- [`bbe6419`](https://github.com/noctisynth/semifold/commit/bbe6419bba673fc0e8a1ab7957d62fd0956b27ed): Skip publish private packages.

### Performance Improvements

- [`25e643d`](https://github.com/noctisynth/semifold/commit/25e643d3c636c409350ec3214ff148558ee486dc): Use `generate-lockfile --offline` instead of `check` to improve post performance.

## v0.1.10

### New Features

- [`235d5f0`](https://github.com/noctisynth/semifold/commit/235d5f0e94b09094abb87caacd93bda46875121a): Support customize standard outputs for `stdout` and `stderr`.

### Bug Fixes

- [`717539f`](https://github.com/noctisynth/semifold/commit/717539f37698d4a8383e21311730bcfa611885e9): Run post version commands in ci environments.

## v0.1.9

### Bug Fixes

- [`6aa9bdf`](https://github.com/noctisynth/semifold/commit/6aa9bdfed57c03ca00bd39d4327409d8ac5087fc): Post version commands should run after all versioning tasks done.
- [`b95d9a5`](https://github.com/noctisynth/semifold/commit/b95d9a5714bb7bd0d4e66a688b0edeb51a34b812): Post version commands run for every package.

## v0.1.8

### Bug Fixes

- [`08b8a47`](https://github.com/noctisynth/semifold/commit/08b8a470f84fdaa2b32b8392b1b4652478023d4f): Fix auto added args for post version commands.

### New Features

- [`08b8a47`](https://github.com/noctisynth/semifold/commit/08b8a470f84fdaa2b32b8392b1b4652478023d4f): Support local versioning.

## v0.1.7

### New Features

- [`979e7de`](https://github.com/noctisynth/semifold/commit/979e7def35be9c1dd527822ab129f534eacec6ef): Support trigger post version commands after versioning.

### Bug Fixes

- [`353f7ee`](https://github.com/noctisynth/semifold/commit/353f7ee50dc81ca9a6f2e67383a9b5178ed5834f): Fix email of CI git config committer.
- [`979e7de`](https://github.com/noctisynth/semifold/commit/979e7def35be9c1dd527822ab129f534eacec6ef): Fix Git username config, use `github-actions[bot]` instead.

## v0.1.6

### Bug Fixes

- [`baa3816`](https://github.com/noctisynth/semifold/commit/baa3816ad6e4312912d368fda83d848b83db20c3): Fix env issue due to actions triggered by `pull_request_target`. ([#19](https://github.com/noctisynth/semifold/pull/19) by @fu050409)
- [`6a82ae3`](https://github.com/noctisynth/semifold/commit/6a82ae3792e0983f4ecd792aaee169d052f8af54): Fix filter pull requests using GitHub API.

### New Features

- [`9674279`](https://github.com/noctisynth/semifold/commit/96742792d4fc8604651feb212dd3f578c2635c16): Optimize status command output message.
- [`450054a`](https://github.com/noctisynth/semifold/commit/450054ad8b496e1634553589d15815b0d8c8048a): add Python support to resolver ([#17](https://github.com/noctisynth/semifold/pull/17) by @HsiangNianian)

## v0.1.5

### New Features

- [`0171573`](https://github.com/noctisynth/semifold/commit/0171573c15463971538c85c801227145e4648e7d): Optimize empty config fields default serialization.

## v0.1.4

### New Features

- [`a6229ae`](https://github.com/noctisynth/semifold/commit/a6229ae83fe10204bc5475320b15bc5e9edf66e7): Add help messages for package selection.
- [`cdc749c`](https://github.com/noctisynth/semifold/commit/cdc749cab0e8e1f390f13f521b7be4041b663740): Support Nodejs workspace resolve and version bumps.

## v0.1.3

### New Features

- [`66da4e2`](https://github.com/noctisynth/semifold/commit/66da4e2d6c26f8abe710f6a231b623127f3be090): Support pre-check config before publishing packages.
- [`3a031ee`](https://github.com/noctisynth/semifold/commit/3a031ee7001923932f1ed6853bfd26e7fd431318): Embed semifold GitHub Actions workflow files.

### Bug Fixes

- [`3a031ee`](https://github.com/noctisynth/semifold/commit/3a031ee7001923932f1ed6853bfd26e7fd431318): Fix resolved project path of single crate project.

## v0.1.2

### New Features

- [`5386984`](https://github.com/noctisynth/semifold/commit/538698464bba9f459b38aaa4cb414112716a2e2d): Add GitHub release title.

### Bug Fixes

- [`c4952cf`](https://github.com/noctisynth/semifold/commit/c4952cff31ed999e44210ffe8dddfcd65f9a526a): Ask user for whether continue due to incomplete selection.
- [`2448ba4`](https://github.com/noctisynth/semifold/commit/2448ba4e59db85c912314d5bfab31784e945980d): Skip unchanged packages when generating changelogs.
- [`5e1b994`](https://github.com/noctisynth/semifold/commit/5e1b994178fa662b630d700559cc888892b44813): Fix path of resolved package is relative path.

## v0.1.1

### Bug Fixes

- [`2245ab9`](https://github.com/noctisynth/semifold/commit/2245ab96d869e5220d125f440747e035774a8c02): Fix `ci` command don't create GitHub releases.
- [`2245ab9`](https://github.com/noctisynth/semifold/commit/2245ab96d869e5220d125f440747e035774a8c02): Fix packages release order.

### Refactors

- [`2eb3d67`](https://github.com/noctisynth/semifold/commit/2eb3d67a373a55104562f2eaee7c6ebd33794510): Rewrite init command to support new configs.

### New Features

- [`d94df17`](https://github.com/noctisynth/semifold/commit/d94df1729f43bf6f159a00ed701e05e75aad2d02): Support create and apply changeset.
- [`9174302`](https://github.com/noctisynth/semifold/commit/9174302d76386cabb8de0948729b1e7267cc8e8f): Support `ci` and `status` command. ([#8](https://github.com/noctisynth/semifold/pull/8) by @fu050409)
- [`4007f78`](https://github.com/noctisynth/semifold/commit/4007f789aabf1aecaccb2066899b148edcd8c24b): Support `version` cli command.
- [`7573c58`](https://github.com/noctisynth/semifold/commit/7573c588702f6e8944ecc53999d62a2cdbfa8f67): Support generate changelog.
- [`9cb72e1`](https://github.com/noctisynth/semifold/commit/9cb72e17d8ca486fc0c4090abeddf8c35eb89e6d): Support create GitHub releases while publishing.
- [`475cc70`](https://github.com/noctisynth/semifold/commit/475cc70a2a373a74e844401cda937af194d22ae2): Add i18n translation for commit cli command.
- [`4ba79d7`](https://github.com/noctisynth/semifold/commit/4ba79d70775fb5f46eb3001c8c7dbce494fa5e54): Support Support changeset file name sanitize.
- [`166ea37`](https://github.com/noctisynth/semifold/commit/166ea37e3cec9c690c0d23eec8c09067d8d9d38c): Auto generate changelog content while running version command.
