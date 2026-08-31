# 02 — UI & Upstream Memory

## UI authority

Quadrant's visual baseline is **not primarily WPF-UI**. The primary implementation reference is the handwritten Slint Fluent-style UI in:

`https://github.com/owu/wsl-dashboard`

Baseline used for this project plan:

- commit: `948589a255a4bd8a3ff9c3de49e2e13109378fcd`
- tag/version behavior: v0.11.0
- license: GPL-3.0-only

Quadrant is also GPL-3.0-only, so relevant upstream Slint code may be copied and modified directly with attribution preserved.

## High-value upstream files

### `src/ui/components/sidebar.slint`

Direct foundation for Quadrant Sidebar.

Important current behavior:

- `collapsed` state
- width transition: compact ~54 px, expanded ~200 px
- ~200 ms ease-out width animation
- hamburger/toggle row
- `SidebarItem` reuse
- primary items at top
- stretch spacer
- divider/utility items at bottom
- theme-driven hover/selected/accent states

Quadrant adaptation:

Primary items:

1. Quadrants
2. Today
3. Focus
4. Review

Footer:

5. Completed
6. Settings
7. About

Remove WSL-only actions/features and rename generic component APIs where useful.

### `src/ui/theme.slint`

Use as the baseline for centralized Fluent-ish theme tokens and global style state.

Keep/derive concepts such as:

- `background`
- `content_bg`
- `sidebar_bg`
- `card_bg`
- primary/secondary text
- border
- hover/selected
- accent
- input colors
- scrollbar colors
- semantic icon colors
- dark/system theme state

Do not carry WSL data structs from this file into Quadrant. Extract visual/global concerns from product-specific data definitions.

### Other useful upstream files

- `components/common.slint`
- `components/title_bar.slint`
- `components/scrollbar.slint`
- `components/modal_manager.slint`
- `components/form_widgets.slint`
- `constants.slint`

Review each file before copying; take visual primitives, not WSL domain semantics.

## Provenance rule for copied files

A directly derived file should keep the upstream SPDX lines and add Quadrant modification attribution if appropriate, for example conceptually:

```text
SPDX-FileCopyrightText: Copyright (c) 2026 owu <...>
SPDX-FileCopyrightText: Copyright (c) 2026 Quadrant contributors
SPDX-License-Identifier: GPL-3.0-only
```

Do not delete upstream copyright information.

Record source file, source commit, destination file, and modification summary in `09_SOURCE_MAP.md`.

## UI structure target

```text
ui/
├─ app.slint
├─ theme.slint
├─ constants.slint
├─ icons.slint
├─ components/
│  ├─ sidebar.slint
│  ├─ common.slint
│  ├─ title_bar.slint
│  ├─ scrollbar.slint
│  ├─ modal_manager.slint
│  ├─ task_card.slint
│  ├─ task_editor.slint
│  ├─ quick_add.slint
│  └─ focus_controls.slint
└─ views/
   ├─ quadrants.slint
   ├─ today.slint
   ├─ focus.slint
   ├─ review.slint
   ├─ completed.slint
   ├─ settings.slint
   └─ about.slint
```

Do not allow `app.slint` to grow into a 70k+ line monolith. Keep app composition at the root and feature visuals in views/components.

## Fluent style principles

- calm neutral surfaces
- clear selection/accent line/background
- restrained corner radii
- compact desktop density
- hover feedback without excessive animation
- consistent 4/8-based spacing rhythm
- strong keyboard usability
- dark/light parity
- Fluent-style iconography

The objective is to visibly belong to the same design family as wsl-dashboard while being recognizably Quadrant.

## Icons and cross-platform requirement

Upstream wsl-dashboard currently uses `Segoe Fluent Icons` glyphs. That is acceptable as a Windows implementation detail but **not as Quadrant's sole icon source**.

Quadrant should define semantic icon IDs such as:

```text
Icon.Quadrants
Icon.Today
Icon.Focus
Icon.Review
Icon.Completed
Icon.Settings
Icon.About
Icon.Add
Icon.Edit
Icon.Delete
Icon.Check
```

The Slint layer consumes those IDs/assets without depending on Windows glyph codepoints.

Prefer a vendored, legally redistributable Fluent-style SVG set and keep its exact license/provenance in `09_SOURCE_MAP.md` / third-party notices.

## Window/backdrop

The old WPF app used a Windows Mica Fluent window. The rewrite may use native Windows backdrop integration from `quadrant-platform`, but core UI must remain valid without Mica.

On platforms without Mica, render the theme surfaces normally. Never fork the entire UI per OS merely to reproduce a backdrop effect.

## Quick Add

Quick Add should visually share theme/components with the main app but remain a minimal window.

Keyboard flow is more important than decorative complexity:

- open
- focus title
- type
- optional lightweight classification/planning
- Enter submit
- Escape cancel

## UI state ownership

`.slint` owns transient interaction state such as hover, collapsed sidebar, selected local control, dialog visibility, and animation.

Rust/application owns business state such as tasks, completion, reminders, focus session truth, settings persistence, and current navigation route when it affects app behavior.

## Implemented M0 baseline

- `ui/theme.slint`, `ui/constants.slint`, `ui/components/common.slint`, and `ui/components/sidebar.slint` are derived/adapted from the pinned wsl-dashboard commit with preserved SPDX attribution.
- The sidebar retains the 54/200px widths, 200ms ease-out animation, top primary group, spacer, footer group, hover/selected states, and accent indicator.
- `ui/icons.slint` exposes semantic image properties backed by vendored Microsoft Fluent UI System Icons SVGs rather than Segoe glyph codepoints.
- `ui/app.slint` is a compiled shell with all seven navigation routes and placeholder content. It is intentionally not the final M1 page implementation.
- Theme tokens support a dark/light boolean foundation, but System/Light/Dark application orchestration is not implemented yet.
