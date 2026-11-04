semifold: patch:fix
"@semifold/cli": patch:fix
semifold-engine: minor:refactor
semifold-changelog: minor:refactor
semifold-resolver: patch:fix
---

Use official gix for all local Git operations, including directory-wildcard trust, worktree status, changeset history, and CI release commits. Keep libgit2 isolated in the CI push adapter, preflight it before release writes, and preserve Git error diagnostics in the CLI and MCP. Local operations do not require an installed Git executable; CI push retains libgit2's directory-wildcard limitation. Changelog collection and engine Git errors now expose gix types, and history lookup returns errors explicitly.

Keep plugin file-list protocol paths slash-separated on Windows so discovered manifests can be read through the same scoped host API.
