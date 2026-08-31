# 00 — Project Memory

Last updated: **2026-08-31**

## Project identity

- Product: **Quadrant Tasks** / **Quadrant**
- Rewrite language: **Rust 2024**
- UI: **Slint**
- Persistence: **new SQLite schema via `rusqlite`**
- License target: **GPL-3.0-only**
- Product model: local-first four-quadrant task manager with Today, Focus, Review, Completed, Quick Add, reminders and desktop integration

## Rewrite status

M0 is complete. The repository now contains a Rust 2024 workspace with all six target crates, a compiled Slint application shell, GPL-3.0-only licensing, provenance records, cross-platform SVG icons, and baseline CI. The legacy .NET 10/WPF implementation remains under `legacy/dotnet-reference/` as a read-only product reference.

There is **no need to migrate or remain compatible with the old database**, because the application has not been released to users.

## Non-negotiables

- No C#/.NET in the final active application.
- No Tauri/Electron/webview UI.
- Rust + Slint throughout.
- Direct GPL-compatible derivation from `owu/wsl-dashboard` Slint UI is allowed and desired.
- The wsl-dashboard Sidebar is the primary sidebar baseline.
- New database/schema is designed for the Rust implementation using `rusqlite`.
- Windows native integration is centralized in the platform crate.
- Cross-platform architecture is first-class; Windows is the first parity implementation.
- Reminder/background behavior is event/deadline driven, not periodic polling.
- No formal Performance Budget project is required.

## Upstream UI baseline

`owu/wsl-dashboard`:

- baseline commit: `948589a255a4bd8a3ff9c3de49e2e13109378fcd`
- version: `0.11.0`
- date: `2026-08-25`
- license: `GPL-3.0-only`

Most important upstream assets: Sidebar, Theme, common controls, title bar, scrollbar, modal system.

## Main navigation baseline

Primary:

- Quadrants
- Today
- Focus
- Review

Footer:

- Completed
- Settings
- About

Quick Add is a separate capture surface/global shortcut target.

## Current architectural target

Workspace crates:

- `quadrant-domain`
- `quadrant-application`
- `quadrant-storage`
- `quadrant-platform`
- `quadrant-ui`
- `quadrant-app`

UI source lives under root `ui/`; migrations under root `migrations/`; persistent agent memory under `docs/`.

The workspace uses Cargo resolver 3, declares Rust 1.92 as its MSRV, pins the repository developer toolchain to stable Rust 1.94.1, checks rolling stable in CI, and pins Slint/`slint-build` to 1.17.1 for the initial shell.

The first implemented boundary types are `TaskPlacement`/`Quadrant` in `quadrant-domain`, `NavigationRoute`/`UiIntent` in `quadrant-application`, and `PlatformCapabilities` in `quadrant-platform`. These are foundations, not the M2 domain implementation.

## Current phase

**M1 — UI shell**, in progress. The window, sidebar, route switching, theme tokens, and icon abstraction exist; title bar, System/Light/Dark orchestration, shared overlays, and Quick Add shell remain.

Immediate implementation sequence is documented in `06_PROGRESS.md` and `08_SESSION_HANDOFF.md`.
