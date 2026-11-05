# Changelog

## v0.1.1

### Chores

- [`dbf3de7`](https://github.com/noctisynth/semifold/commit/dbf3de77f994a5ba092c419661f7e0013765c993): Refresh official stable Rust and Bun workspace dependencies and lockfiles, including Boa 0.22, MiniJinja 3, saphyr 0.3, N-API, and the documentation toolchain. Adapt explicit template serialization, YAML nodes, fallible Promise resolution, and MCP server configuration while preserving plugin permissions, resource budgets, strict templates, protocol schema v1, and the local gix / git2 push boundary. Migrate Biome configuration with its official CLI. ([#118](https://github.com/noctisynth/semifold/pull/118) by @fu050409)

    Regenerate the N-API loader and declarations using the upgraded official CLI and verify the Windows native module.

