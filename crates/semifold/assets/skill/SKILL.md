---
name: semifold
description: Use Semifold (smif) to adopt release management, create changesets, inspect release plans, maintain package configuration, or diagnose version and publish workflows in a user's repository.
---

# Semifold

Use the intended installed `smif` (or `semifold`) executable for discovery and execution. If it is unavailable, report the missing prerequisite and follow the user's installation constraints.

1. Run `smif skill list` to discover the tasks supported by that executable.
2. Select relevant task IDs from the returned summaries and run `smif skill get TASK`. Follow the returned `instructions`; `commands` lists related command paths.
3. Use `smif <command> --help` when exact arguments are needed, then execute the requested task with the same executable.

Discovery returns JSON with `protocolVersion`, `cliVersion`, `language`, and `data`. This entry supports protocol 1. Use `data` for task results. For Chinese requests, add `--language zh` to skill discovery and respond in Chinese; use `--language en` for English guidance.

Reuse the retrieved guidance during the task. Refresh it when the executable or its version changes. A failed discovery command or unsupported protocol is a compatibility issue to resolve; do not guess task IDs or substitute instructions from another version, a newer checkout, or adjacent reference files.

Follow the target repository's instructions and the user's authorized scope. Creating release records does not authorize pushing, merging, or publishing. This skill guides use of Semifold; contributions to Semifold itself follow its repository instructions.
