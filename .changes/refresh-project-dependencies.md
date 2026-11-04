semifold: patch:chore
"@semifold/cli": patch:chore
"@semifold/plugin-sdk": patch:chore
semifold-core: patch:chore
semifold-engine: patch:chore
semifold-changelog: patch:chore
semifold-resolver: patch:chore
semifold-napi: patch:chore
---

Refresh official stable Rust and Bun workspace dependencies and lockfiles, including Boa 0.22, MiniJinja 3, saphyr 0.3, N-API, and the documentation toolchain. Adapt explicit template serialization, YAML nodes, fallible Promise resolution, and MCP server configuration while preserving plugin permissions, resource budgets, strict templates, protocol schema v1, and the local gix / git2 push boundary. Migrate Biome configuration with its official CLI.

Regenerate the N-API loader and declarations using the upgraded official CLI and verify the Windows native module.
