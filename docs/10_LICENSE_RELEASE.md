# 10 — License & Release Memory

## Project license

Quadrant rewrite target license: **GNU General Public License v3.0 only (GPL-3.0-only)**.

Reason: Quadrant intentionally derives Slint UI code from `owu/wsl-dashboard`, whose relevant source files use `SPDX-License-Identifier: GPL-3.0-only`.

The old repository's MIT license is not the rewrite license baseline.

M0 switched the root `LICENSE` to the complete GPL v3 text and added `THIRD-PARTY-NOTICES.md`.

## GPL source derivation rules

For copied/modified upstream source:

- retain upstream copyright/SPDX header
- retain GPL-3.0-only SPDX identifier
- add Quadrant modification copyright if appropriate
- record file mapping and upstream commit in `09_SOURCE_MAP.md`
- keep source available with distributed GPL binaries as required by the distribution model

Do not copy code from another project with incompatible licensing merely because Quadrant itself is GPL.

## Third-party notices

Maintain a root notice file (recommended `THIRD-PARTY-NOTICES.md`) containing at least:

- wsl-dashboard acknowledgement and repository
- copied/derived file families
- icon asset provenance/license
- other bundled assets requiring notices

Crate dependencies are still governed by their own licenses; verify release compliance before packaging.

The first bundled asset dependency is Microsoft Fluent UI System Icons (MIT), pinned to commit `4d685f77b2cb8f3f412a74ec8d920c8c91149528`. Its license is copied to `assets/icons/LICENSE-MIT`; selected asset details are in `09_SOURCE_MAP.md`.

## About page

About should visibly include:

- GPL-3.0-only
- Quadrant repository
- third-party notices link
- acknowledgement of wsl-dashboard UI derivation

## Release channels

Legacy Quadrant targeted GitHub installers and Microsoft Store. The Rust rewrite is cross-platform, so release design should separate application logic from packaging.

Potential channels are decided per platform later:

- Windows: GitHub Releases and optional Store/WinGet-compatible packaging
- Linux: package/AppImage/etc. decision later
- macOS: app bundle/DMG/notarization decision later

Do not make one store-specific API an application-layer dependency.

## Update architecture

Self-update is not required for M0-M2. When implemented:

- isolate update checking/downloading from UI
- respect distribution channel capabilities
- do not replace a running executable in an unsafe way
- use a companion updater only if packaging/platform semantics require it

## Versioning

Use Cargo/package version as canonical application version unless a later release system requires generated platform metadata.

Keep version information single-source as much as practical.
