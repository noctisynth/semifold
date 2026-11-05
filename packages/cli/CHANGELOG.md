# Changelog

## v0.3.5

### Bug Fixes

- [`bcde56a`](https://github.com/noctisynth/semifold/commit/bcde56aeb7737dd71f4a4f948f265f934c98214b): Use official gix for all local Git operations, including directory-wildcard trust, worktree status, changeset history, and CI release commits. Keep libgit2 isolated in the CI push adapter, preflight it before release writes, and preserve Git error diagnostics in the CLI and MCP. Local operations do not require an installed Git executable; CI push retains libgit2's directory-wildcard limitation. Changelog collection and engine Git errors now expose gix types, and history lookup returns errors explicitly. ([#118](https://github.com/noctisynth/semifold/pull/118) by @fu050409)

    Keep plugin file-list protocol paths slash-separated on Windows so discovered manifests can be read through the same scoped host API.


### Chores

- [`dbf3de7`](https://github.com/noctisynth/semifold/commit/dbf3de77f994a5ba092c419661f7e0013765c993): Refresh official stable Rust and Bun workspace dependencies and lockfiles, including Boa 0.22, MiniJinja 3, saphyr 0.3, N-API, and the documentation toolchain. Adapt explicit template serialization, YAML nodes, fallible Promise resolution, and MCP server configuration while preserving plugin permissions, resource budgets, strict templates, protocol schema v1, and the local gix / git2 push boundary. Migrate Biome configuration with its official CLI. ([#118](https://github.com/noctisynth/semifold/pull/118) by @fu050409)

    Regenerate the N-API loader and declarations using the upgraded official CLI and verify the Windows native module.


### Dependencies

- Update semifold to 0.3.5.

## v0.3.4

### Dependencies

- Update semifold to 0.3.4.

## v0.3.3

### Dependencies

- Update semifold to 0.3.3.

## v0.3.2

### Dependencies

- Update semifold to 0.3.2.

## v0.3.1

### New Features

- [`1b2317a`](https://github.com/noctisynth/semifold/commit/1b2317add403852d475ba95c4a0c0ebe438cdcd8): Publish Semifold as an npm-installed Node.js CLI backed by napi-rs generated native packages for supported macOS, Linux glibc, and Windows targets.

### Dependencies

- Update semifold to 0.3.1.
