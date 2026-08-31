# 05 — Decision Log

This is an append-oriented lightweight ADR log. Do not erase old decisions; mark them **Superseded** when replaced.

## D-001 — Full Rust + Slint rewrite

**Status:** Accepted  
**Date:** 2026-08-31

Quadrant will be rewritten completely using Rust + Slint. The new application will not retain a C#/.NET runtime or compatibility layer.

## D-002 — Legacy C# is reference-only

**Status:** Accepted  
**Date:** 2026-08-31

The .NET 10/WPF implementation may be inspected to recover product behavior and appearance. It is not the new architecture and should not receive new feature work. Final active application source is Rust/Slint only.

## D-003 — New rusqlite schema, no legacy DB migration

**Status:** Accepted  
**Date:** 2026-08-31

The application has not shipped; therefore the rewrite will design a clean database schema from scratch using SQLite through `rusqlite`. No legacy database migration/import work is required unless explicitly requested later.

## D-004 — wsl-dashboard is the primary UI upstream

**Status:** Accepted  
**Date:** 2026-08-31

The handwritten Fluent-style Slint UI of `owu/wsl-dashboard` is the primary UI source/reference, especially its Sidebar. Relevant UI code may be copied and modified directly rather than merely imitated.

Baseline upstream snapshot: `948589a255a4bd8a3ff9c3de49e2e13109378fcd` (v0.11.0).

## D-005 — Quadrant becomes GPL-3.0-only

**Status:** Accepted  
**Date:** 2026-08-31

Quadrant will use GPL-3.0-only, matching the directly derived wsl-dashboard code. Upstream SPDX/copyright notices must be preserved.

## D-006 — Cross-platform is an architectural requirement

**Status:** Accepted  
**Date:** 2026-08-31

Windows is the first functional parity target, but UI/domain/application/storage must not be Windows-bound. OS integration is isolated in `quadrant-platform` with target-specific implementations.

## D-007 — Windows native code is centralized

**Status:** Accepted  
**Date:** 2026-08-31

All direct Windows-native behavior/APIs live in the platform crate. No Win32 calls in UI, domain, storage, or general application modules.

## D-008 — No formal Performance Budget milestone

**Status:** Accepted  
**Date:** 2026-08-31

The rewrite is expected to improve runtime characteristics by architecture/technology choice. Formal startup/RSS benchmark gates are not part of the requested project plan. Correctness and event-driven idle behavior remain required.

## D-009 — Do not make Slint backend/renderer trimming a project goal

**Status:** Accepted  
**Date:** 2026-08-31

Do not start the rewrite by disabling default Slint features solely to minimize compiled backends/renderers. Use a normal supported Slint setup; renderer/backend tuning is a later implementation detail if actually needed.

## D-010 — Persistent docs are mandatory agent memory

**Status:** Accepted  
**Date:** 2026-08-31

Codex must read the designated `docs/` memory files at the beginning of a new working session and update the handoff/progress/decision memory before ending substantive work. `AGENTS.md` enforces the protocol.

## D-011 — Event-driven reminder scheduling

**Status:** Accepted  
**Date:** 2026-08-31

Reminder handling is based on nearest-deadline waiting plus mutation signals. Periodic database polling is prohibited.

## D-012 — UI icon source must be cross-platform

**Status:** Accepted  
**Date:** 2026-08-31

Quadrant may reproduce the Fluent icon language of wsl-dashboard but must not rely solely on Windows-installed `Segoe Fluent Icons`. Essential icons are routed through a cross-platform asset abstraction with recorded licensing/provenance.

## D-013 — Stable Rust with explicit MSRV

**Status:** Accepted  
**Date:** 2026-08-31

Quadrant pins the repository developer toolchain to stable Rust 1.94.1 through `rust-toolchain.toml`, while CI also checks the current stable channel. Workspace packages declare Rust 1.92 as the initial minimum supported Rust version, matching the pinned wsl-dashboard baseline's toolchain floor. `Cargo.lock` is committed for reproducible application builds.

## D-014 — Pin initial Slint pipeline to 1.17.1

**Status:** Accepted  
**Date:** 2026-08-31

The M0 UI pipeline pins `slint` and `slint-build` to 1.17.1, the version used by the pinned wsl-dashboard v0.11.0 baseline. The normal supported Slint feature configuration is used; backend/renderer trimming is not part of M0.

## D-015 — Microsoft Fluent UI System Icons for semantic SVG assets

**Status:** Accepted  
**Date:** 2026-08-31

Quadrant's first cross-platform icon assets come from Microsoft Fluent UI System Icons commit `4d685f77b2cb8f3f412a74ec8d920c8c91149528` (release 1.1.339), licensed MIT. The UI consumes them through semantic properties in `ui/icons.slint`; exact files and license copies are recorded in `09_SOURCE_MAP.md` and `THIRD-PARTY-NOTICES.md`.
