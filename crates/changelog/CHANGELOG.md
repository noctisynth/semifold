# Changelog

## v0.3.2

### Bug Fixes

- [`1177d67`](https://github.com/noctisynth/semifold/commit/1177d671fbdcb9c5b7be50ed987c480ee9508f77): Preserve and localize GitHub operation, API status, validation details, and underlying errors across CI pull requests, publishing releases and assets, status queries and comments, and changelog metadata lookup while retaining existing failure and recovery behavior.

## v0.3.1

### Chores

- [`05a2e7c`](https://github.com/noctisynth/semifold/commit/05a2e7c6b85697cbd091f74698d88a87f8678f01): Upgrade Rust and JavaScript dependencies to their latest compatible versions, including the required API migrations for Git, HTTP, hashing, localization, MCP, and configuration handling.

## v0.3.0

### Chores

- [`28ee39d`](https://github.com/noctisynth/semifold/commit/28ee39d4295b91d1afc8614eec0ed6327ca22af7): Promote the remaining prerelease packages to their stable release channel.

## v0.3.0-rc.2

### Bug Fixes

- [`9e042d4`](https://github.com/noctisynth/semifold/commit/9e042d469769d6f0a74537a359c417028ded2330): Normalize source-wrapped lines within each changeset paragraph to spaces in the default changelog template while preserving explicit blank-line paragraph boundaries and custom template access to the original summary structure.

## v0.3.0-rc.1

### New Features

- [`2f843f3`](https://github.com/noctisynth/semifold/commit/2f843f38b196bf61cffd41daa6ae07cdccd6fe94): Allow workspace owners to customize complete changelog release blocks and individual changeset entries with strict MiniJinja templates. Template contexts now expose structured summaries and commit metadata, while stable release markers let publish consume arbitrary rendered formats without breaking legacy changelogs.
- [`42f88c9`](https://github.com/noctisynth/semifold/commit/42f88c9302f8d974c8351aaa30297b0aff2fb002): Write the built-in release and changeset changelog templates into newly initialized configuration files so users can discover and customize them immediately, while retaining the same templates as fallbacks for older configurations.

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

## v0.3.0-beta.2

### New Features

- [`2624d2d`](https://github.com/noctisynth/semifold/commit/2624d2d12fb9678d3f622a2aac69acddbd3af5f4): Model changelog rendering as immutable package and changeset facts.

    Changelog collection now resolves package sections and optional commit and pull request metadata
    before passing a capability-free aggregate context to the Markdown formatter.

## v0.3.0-beta.1

### Bug Fixes

- [`de16b01`](https://github.com/noctisynth/semifold/commit/de16b017c5163a563aee17f6dfc2fd37345ca74a): Render multiline changeset entries as valid nested Markdown paragraphs

    Blank lines in a changeset summary now separate content without producing whitespace-only
    paragraphs. Every non-empty continuation line is emitted after a blank line with four spaces of
    indentation, and regression coverage uses the same front matter and prose shape as real changesets.

- [`9da27d5`](https://github.com/noctisynth/semifold/commit/9da27d5191fe15900de27882e6a6cac0d8061e1c): Preserve wrapped lines within changelog paragraphs

    Changelog formatting now uses blank lines, rather than every physical line break, as paragraph
    boundaries. Hard-wrapped lines remain together inside an indented continuation paragraph.

## v0.3.0-beta.0

### Bug Fixes

- [`90e6cd6`](https://github.com/noctisynth/semifold/commit/90e6cd697eed4686dcfc946eb833ba76bc89c1ff): Render multiline changeset summaries as separate paragraphs in one changelog list item.

## v0.3.0-alpha.2

### Bug Fixes

- [`66edf18`](https://github.com/noctisynth/semifold/commit/66edf18a2445f8543d051e750010a5ab75e9cf91): Continue versioning without pull request details when GitHub changelog metadata lookup fails.
- [`3ef7c05`](https://github.com/noctisynth/semifold/commit/3ef7c0538172b0e3fd4e3755d44dfe5c26255e40): Warn when changelog generation continues without pull request metadata.

### Refactors

- [`2a25a56`](https://github.com/noctisynth/semifold/commit/2a25a562cdaff0917873efa3a9ba3c15a8e4b747): Introduce an immutable changelog formatting context and pure Markdown renderer.
- [`02b0c7c`](https://github.com/noctisynth/semifold/commit/02b0c7c8e4bb2bed8b41091adc1336f10a167462): Separate changelog context collection from pure Markdown formatting.

## v0.3.0-alpha.1

### Bug Fixes

- [`3b961e0`](https://github.com/noctisynth/semifold/commit/3b961e069ea4bd5b43e5b29bf2f5c5fc39414c9b): Return errors instead of panicking when release planning, configuration, changelog, and resolver invariants are unavailable.
- [`efa7e5b`](https://github.com/noctisynth/semifold/commit/efa7e5be09ca8268155a70391ab4a189c603fc9f): Validate changelog edits as part of the immutable release plan before applying version changes, without remote metadata requests during dry runs.

### Refactors

- [`f42b195`](https://github.com/noctisynth/semifold/commit/f42b195058a4f6972f1a7468023c0512f4845d24): Keep recoverable input failures as errors while using documented `expect` calls for verified internal invariants.
- [`56e0686`](https://github.com/noctisynth/semifold/commit/56e06863fb2846497ed0b79417d68f3cf17eb8ca): Enforce the production unwrap policy through shared workspace Clippy configuration while allowing documented internal expects.

### New Features

- [`09e7af6`](https://github.com/noctisynth/semifold/commit/09e7af6e76bed7a01b72e6a675504ab308732d2f): Plan Rust and Node changelog updates as validated file edits that can safely create a missing changelog.

## v0.3.0-alpha.0

### New Features

- [`7b59b5d`](https://github.com/noctisynth/semifold/commit/7b59b5dea42b8e4feeb9c3d3887e06c0c8ce950d): Generate Dependencies changelog entries for packages automatically released because an internal dependency changed.

## v0.2.1

### New Features

- [`d8959d0`](https://github.com/noctisynth/semifold/commit/d8959d02b980e2407fa95009e8afbf4c4375b1c0): 1. Add base_url field to RepoInfo struct 2. Read GITHUB_SERVER_URL env var with fallback to <https://github.com> 3. Use dynamic URL in changelog commit links ([#58](https://github.com/noctisynth/semifold/pull/58) by @BegoniaHe)

## v0.2.0

### New Features

- [`e009c7e`](https://github.com/noctisynth/semifold/commit/e009c7ec0d2908cdf6bf11430a7c0db46f8f40ad): Support running commands in dry run mode.

## v0.1.7

### New Features

- [`4856c7d`](https://github.com/noctisynth/semifold/commit/4856c7d14bb2bd3622f9ae29f8b75e5ad2f60165): Improve compatibility to `changesets` and `covector`, allow empty tag key now.

## v0.1.6

### Performance Improvements

- [`940c9fc`](https://github.com/noctisynth/semifold/commit/940c9fcfb0422fd98e239401b01683945011227e): Disable useless features and create release profile for binary size optimizations.

## v0.1.5

### Chores

- [`dccb0d2`](https://github.com/noctisynth/semifold/commit/dccb0d2312ea31e340a67ab2f6552a3918ce887a): Add readme and authors fields to `Cargo.toml`.

## v0.1.4

### New Features

- [`450054a`](https://github.com/noctisynth/semifold/commit/450054ad8b496e1634553589d15815b0d8c8048a): add Python support to resolver ([#17](https://github.com/noctisynth/semifold/pull/17) by @HsiangNianian)

## v0.1.3

### New Features

- [`cdc749c`](https://github.com/noctisynth/semifold/commit/cdc749cab0e8e1f390f13f521b7be4041b663740): Support Nodejs workspace resolve and version bumps.

## v0.1.2

### Bug Fixes

- [`d994409`](https://github.com/noctisynth/semifold/commit/d99440975a113c984131983c5a6e148e481d2c9b): Fix format of commit hashes in changelog.

## v0.1.1

### New Features

- [`7573c58`](https://github.com/noctisynth/semifold/commit/7573c588702f6e8944ecc53999d62a2cdbfa8f67): Support generate changelog.
- [`9cb72e1`](https://github.com/noctisynth/semifold/commit/9cb72e17d8ca486fc0c4090abeddf8c35eb89e6d): Support create GitHub releases while publishing.
- [`166ea37`](https://github.com/noctisynth/semifold/commit/166ea37e3cec9c690c0d23eec8c09067d8d9d38c): Auto generate changelog content while running version command.
