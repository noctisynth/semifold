# Changelog

## v0.2.3

### Bug Fixes

- [`1177d67`](https://github.com/noctisynth/semifold/commit/1177d671fbdcb9c5b7be50ed987c480ee9508f77): Preserve and localize GitHub operation, API status, validation details, and underlying errors across CI pull requests, publishing releases and assets, status queries and comments, and changelog metadata lookup while retaining existing failure and recovery behavior.

## v0.2.2

### Bug Fixes

- [`1bda032`](https://github.com/noctisynth/semifold/commit/1bda03243247d168877cee92f449ea9ed9e800a4): Honor the global dry-run boundary for changeset creation, initialization, and pull-request status comments.
- [`c47ccd5`](https://github.com/noctisynth/semifold/commit/c47ccd5786ab38391efc8ca09cd3ab8cd4477960): Reject release branches that match the base branch during configuration loading, initialization, and rendered release execution before Semifold can force-update the base branch.

## v0.2.1

### Chores

- [`05a2e7c`](https://github.com/noctisynth/semifold/commit/05a2e7c6b85697cbd091f74698d88a87f8678f01): Upgrade Rust and JavaScript dependencies to their latest compatible versions, including the required API migrations for Git, HTTP, hashing, localization, MCP, and configuration handling.

### New Features

- [`3454415`](https://github.com/noctisynth/semifold/commit/34544159c278e1c9096e1e002ab6966f978300e6): Allow release commit messages and pull request titles to use optional strict release context templates while preserving the existing defaults.

## v0.2.0

### New Features

- [`c64a001`](https://github.com/noctisynth/semifold/commit/c64a0011732bb40044d9486ef708fb03bfc9c650): Add an optional package-level `publish` override that defaults to manifest or plugin discovery and explicitly controls registry pre-check and publish command eligibility.
- [`61697f3`](https://github.com/noctisynth/semifold/commit/61697f3ffb25614e8633b041cfd1b96effa6632c): Make `smif init` assign ecosystem-prefixed IDs to packages that share a manifest name across ecosystems, with deterministic numeric suffixes when needed.
- [`d8d7bc1`](https://github.com/noctisynth/semifold/commit/d8d7bc110712743ab35b83fa861ddcc1a66812d1): Suggest `smif config migrate` when a TOML configuration cannot be loaded under the current contract, while keeping unrelated project and format errors focused on their actual cause.

## v0.2.0-rc.4

### Bug Fixes

- [`9b3eb89`](https://github.com/noctisynth/semifold/commit/9b3eb89ebd4460c2bcfed5fd73daafe3ff27b069): Use `nodejs` as the canonical built-in ecosystem identity so domain serialization, existing package configuration, adapter lookup, and future plugin registration share one stable value without aliases.
- [`b17acdb`](https://github.com/noctisynth/semifold/commit/b17acdb420cd9d02e6f85716219f6eba3dba899a): Run config migrate against raw TOML before strict project loading, and warn when channel updates target Node.js packages whose npm publish command lacks an explicit dist-tag.
- [`92cf6d7`](https://github.com/noctisynth/semifold/commit/92cf6d70e7f516965461cbd5801b5b1640d27f80): Continue a planned GitHub Release after registry preflight finds an existing package version, while keeping registry commands skipped, reporting both outcomes, and leaving existing Release assets untouched.

### New Features

- [`ca56371`](https://github.com/noctisynth/semifold/commit/ca56371c162e7845e8f8652d9e2b488021a85de8): Configure repository-local dynamic ecosystem plugins by stable ID with optional SHA-256 pins and exact HTTPS origins, and route discovery, workspace loading, config sync, and version edit planning through their authenticated adapters.
- [`90f27bb`](https://github.com/noctisynth/semifold/commit/90f27bb8bd85a7d0a1dbb6830cfec2ed46ec1152): Replace the MCP changeset surface with lazily loaded, structured get/create/update/delete tools, optimistic SHA-256 revisions, dry-run planning, localized errors, and panic isolation that keeps the stdio server available after failed calls.
- [`8caff0a`](https://github.com/noctisynth/semifold/commit/8caff0ad5ace8ca72600ed1acabd08a6a1ec2b13): Migrate domain packages, plans, contexts, and adapters to open ecosystem identities while preserving built-in serialization and ordering compatibility ahead of dynamic plugin registration.

## v0.2.0-rc.3

### Bug Fixes

- [`b31f342`](https://github.com/noctisynth/semifold/commit/b31f342ef8421d6a312b83099b20d31710ae10ca): Report post-version commands in their actual sequential execution order. Commands with captured output show per-command progress, while commands inheriting the terminal only print a result after their child process exits.

### New Features

- [`b52d510`](https://github.com/noctisynth/semifold/commit/b52d510607c00954cb647f7d3e8b70ee7095f1f5): Expose versioned, allowlisted GitHub Actions outputs from `smif version` and `smif publish`. Version outputs preserve the release plan fingerprint, branch, and package versions, while publish outputs retain complete package recovery states after partial failures without leaking command or environment configuration.

## v0.2.0-rc.2

### New Features

- [`c90d417`](https://github.com/noctisynth/semifold/commit/c90d4173d28e04e951183b3f10bc28905df0df2a): Make HTTP publish pre-checks inject an overridable runtime User-Agent, retry transient failures
    using configured delays and Retry-After, and report bounded response details for registry errors.
- [`7ca5d48`](https://github.com/noctisynth/semifold/commit/7ca5d48f26c5de2f888d89c4b45d8650165b9b74): Limit Semifold configuration to TOML and add typed HTTP or command publish pre-checks. Command
    pre-checks exchange package metadata and existence results through a strict JSON Lines protocol,
    while HTTP checks now fail safely on statuses other than 200 and 404.

## v0.2.0-rc.1

### Bug Fixes

- [`b4871b3`](https://github.com/noctisynth/semifold/commit/b4871b3749d814ddb9b7ca38e84c0e2f46d78e37): Stop forcing Cargo offline mode in the Rust resolver's default post-version lockfile generation command.

### New Features

- [`2f843f3`](https://github.com/noctisynth/semifold/commit/2f843f38b196bf61cffd41daa6ae07cdccd6fe94): Allow workspace owners to customize complete changelog release blocks and individual changeset entries with strict MiniJinja templates. Template contexts now expose structured summaries and commit metadata, while stable release markers let publish consume arbitrary rendered formats without breaking legacy changelogs.
- [`42f88c9`](https://github.com/noctisynth/semifold/commit/42f88c9302f8d974c8351aaa30297b0aff2fb002): Write the built-in release and changeset changelog templates into newly initialized configuration files so users can discover and customize them immediately, while retaining the same templates as fallbacks for older configurations.
- [`145ccec`](https://github.com/noctisynth/semifold/commit/145ccecb196b51bbe40d1123c6b7ad6f4678aa25): Add an optional per-package `github-release` policy. Public packages keep GitHub Releases enabled by
    default, private packages keep them disabled by default, and either default can now be overridden
    explicitly without changing registry publishability.

## v0.2.0-rc.0

### New Features

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

- [`3f71dbc`](https://github.com/noctisynth/semifold/commit/3f71dbc2bae9f069769c87364e716c5d73cd263d): Introduce explicit project discovery and loading models with structured errors, including lossless
    handling of non-UTF-8 operating-system paths.


### Refactors

- [`6838d72`](https://github.com/noctisynth/semifold/commit/6838d72730fc38e389885322637621bba0d2aadd): Introduce explicit domain and application error boundaries and remove `anyhow` from the engine.

    All production targets now reject panic-prone unwraps, expects, indexing, and slicing under strict
    Clippy validation, including workspace planning, Rust manifest edits, changelog metadata parsing,
    configuration editing, and embedded initialization assets.
