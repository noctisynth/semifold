# Changelog

## v1.1.2

### Bug Fixes

- [`1177d67`](https://github.com/noctisynth/semifold/commit/1177d671fbdcb9c5b7be50ed987c480ee9508f77): Preserve and localize GitHub operation, API status, validation details, and underlying errors across CI pull requests, publishing releases and assets, status queries and comments, and changelog metadata lookup while retaining existing failure and recovery behavior.

## v1.1.1

### Bug Fixes

- [`c47ccd5`](https://github.com/noctisynth/semifold/commit/c47ccd5786ab38391efc8ca09cd3ab8cd4477960): Reject release branches that match the base branch during configuration loading, initialization, and rendered release execution before Semifold can force-update the base branch.
- [`c5ff0ce`](https://github.com/noctisynth/semifold/commit/c5ff0ce5d66d596e1b37d77cdd912ec4ed453ecc): Show structured operation, GitHub API status, message, documentation, and permission details when `status` cannot write its pull request comment, and document the repository workflow permissions required by generated GitHub Actions.
- [`cb3d88f`](https://github.com/noctisynth/semifold/commit/cb3d88f3d40b7ee0c9dd2eb3edcf5dafccad0d4b): Resolve installation-script defaults from the latest stable `semifold-vX.Y.Z` binary Release instead of the repository-wide latest Release, and normalize explicit versions with or without a leading `v`.

## v1.1.0

### New Features

- [`61697f3`](https://github.com/noctisynth/semifold/commit/61697f3ffb25614e8633b041cfd1b96effa6632c): Make `smif init` assign ecosystem-prefixed IDs to packages that share a manifest name across ecosystems, with deterministic numeric suffixes when needed.
- [`d8d7bc1`](https://github.com/noctisynth/semifold/commit/d8d7bc110712743ab35b83fa861ddcc1a66812d1): Suggest `smif config migrate` when a TOML configuration cannot be loaded under the current contract, while keeping unrelated project and format errors focused on their actual cause.

## v1.1.0-rc.0

### New Features

- [`174b41d`](https://github.com/noctisynth/semifold/commit/174b41d2b16dbd516a366d78307dc9ad267b5c9e): Reframe Semifold around polyglot monorepo versioning and releases, document configuration and the complete plugin system in both languages, add a localized glossary and site footer, and replace the homepage's custom CSS and placeholder ecosystem badges with Tailwind utilities and official logos.
- [`48f4a96`](https://github.com/noctisynth/semifold/commit/48f4a966ae98abf8c0348a0c83fa958c8137531f): Migrate the documentation foundation to Next.js and Fumadocs with explicit English and Chinese routes, static search and LLM exports, a redesigned homepage, and a verified first-release documentation slice.
- [`48508a6`](https://github.com/noctisynth/semifold/commit/48508a626457124b1f3e5c88a988ed2ffe486f65): Add detailed per-command documentation, update the published baseline to Semifold 0.3.0-rc.6, clarify the GitHub Actions release path, and fix locale switching, responsive overflow, lifecycle visualization, navigation logo rendering, and route-transition scrolling diagnostics.

## v1.1.0-beta.0

### New Features

- [`2d89016`](https://github.com/noctisynth/semifold/commit/2d89016d63d7e4cd0ea670bac09d8896eb024968): Allow the installation scripts to install Semifold into a custom directory

    Unix users can pass `--install-dir`, while Windows users can pass `-InstallDir`. Both scripts keep
    using the user-local binary directory by default and allow a custom directory to be combined with
    a specific version.

- [`2d89016`](https://github.com/noctisynth/semifold/commit/2d89016d63d7e4cd0ea670bac09d8896eb024968): Allow the Unix and Windows installation scripts to install a specific Semifold version

    Passing a version now downloads assets from the matching `semifold-{version}` GitHub release,
    while invoking either script without an argument continues to install the latest release.

## v1.0.6

### Chores

- [`561e98a`](https://github.com/noctisynth/semifold/commit/561e98a8733dfa390b12d1bbdba9746f03d5e1f2): Approve builds of `core-js` by pnpm

## v1.0.5

### Chores

- [`8d6ec13`](https://github.com/noctisynth/semifold/commit/8d6ec138b31560337dbfdc854e61d68072ca9be1): update README to indicate the support for C++ projects, which was forgotten in the previous commits ([#54](https://github.com/noctisynth/semifold/pull/54) by @BegoniaHe)

## v1.0.4

### New Features

- [`61d9a7b`](https://github.com/noctisynth/semifold/commit/61d9a7bc3bc73879a3bdb9e7a59c81581cdbfd72): Add script-based installation method in quick start section.

## v1.0.3

### New Features

- [`549d339`](https://github.com/noctisynth/semifold/commit/549d33903c8731f334305fc1d57f3291f1437f02): Optimize CI template and add shell install scripts.

## v1.0.2

### New Features

- [`bfbe9b8`](https://github.com/noctisynth/semifold/commit/bfbe9b8961ccf370e6cfe6e33924ebf36d026fd6): Add getting started page and introduction page and add i18n support.
- [`bfbe9b8`](https://github.com/noctisynth/semifold/commit/bfbe9b8961ccf370e6cfe6e33924ebf36d026fd6): Move all assets and styles to root dir of docs project.

### Chores

- [`bfbe9b8`](https://github.com/noctisynth/semifold/commit/bfbe9b8961ccf370e6cfe6e33924ebf36d026fd6): Fix docs build due to `.tsx` component.

## v1.0.1

### Chores

- [`333bec2`](https://github.com/noctisynth/semifold/commit/333bec2e0b8540832739fb3e70f9bb6ec9cc6148): Setup Node.js workspace and optimize docs config.
