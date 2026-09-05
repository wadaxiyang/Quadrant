<p align="center">
  <img src="assets/branding/quadrant-mark.svg" width="96" height="96" alt="Quadrant app icon">
</p>

<h1 align="center">Quadrant</h1>

Quadrant is a local-first, cross-platform four-quadrant task manager built with Rust and Slint.

The Rust application includes Quadrants, Today, Focus, Review, Completed history, reminders, Quick Add, desktop integration, and SQLite-consistent backup/restore.

## Build

Quadrant runs a resident background Agent and a disposable Slint GUI. Build both
companions, then open the GUI; it starts the Agent automatically when needed:

```console
cargo build --locked -p quadrant-agent -p quadrant-app
cargo run --locked -p quadrant-app
```

Minimize keeps the GUI on the taskbar. With **Close to tray** enabled, closing
exits the GUI process while reminders and Focus continue in the Agent. The tray
reopens the interface, and **Exit** stops both processes. Turning Close to tray
off makes window Close request a full exit. Startup preferences apply to the
Agent: **Start in background** means login starts no GUI at all.

The GUI communicates only through local IPC and never opens SQLite. Keep both
executables together. For Agent startup-policy testing, use
`cargo run --locked -p quadrant-agent -- --background`.

For both optimized executables:

```console
cargo build --locked --release -p quadrant-agent -p quadrant-app
```

The Fluent component Gallery is an independent development tool and is not part of
the Product build:

```console
cargo run --locked -p quadrant-ui-gallery
```

Use `scripts/capture_gallery_baseline.ps1` to create its deterministic renderer
snapshot smoke or matrix outputs.

The UI dependency guard is enforced and uses only the Python standard library:

```console
python scripts/check_ui_boundaries.py
```

It fails on public-API drift, duplicate component definitions, obsolete facade
paths, invalid Kit/Gallery/Product imports, forbidden Cargo dependencies, or
missing Slint SPDX headers.

The Gallery provides keyboard-accessible catalog navigation, category filters,
Light/Dark/System themes, Compact/Medium/Wide preview containers, live component
properties, accessibility notes, and Kit-only code samples. Snapshot automation
may select a routed page with `QUADRANT_GALLERY_PAGE=0..8` and a preview width with
`QUADRANT_GALLERY_PREVIEW=0..2`.

Windows build outputs are `target/release/quadrant-agent.exe` and
`target/release/quadrant-app.exe`. The scripts under `packaging/windows/`,
`packaging/linux/` and `packaging/macos/` ship both companions together, with the
user-facing GUI named `quadrant` (`quadrant.exe` on Windows). Run the GUI from the
complete extracted package. Executable discovery does not depend on the working
directory. The macOS bundle includes both programs under `Contents/MacOS`.
Linux/macOS packages require their native build/signing hosts.

Quadrant stores its local database as `quadrant-rust.db` in the platform application-data directory. Settings can create validated backups and stage the latest backup for restore on the next Agent startup. Use tray Exit and reopen Quadrant to apply a staged restore; closing only the GUI keeps the Agent running. The previous live database is retained under the adjacent `recovery` directory.

The project is licensed under [GPL-3.0-only](LICENSE). UI primitives are derived from [`owu/wsl-dashboard`](https://github.com/owu/wsl-dashboard), and bundled icons come from [Microsoft Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons). Release notices are maintained in [`packaging/THIRD-PARTY-NOTICES.txt`](packaging/THIRD-PARTY-NOTICES.txt), with the locked Rust package inventory in [`packaging/DEPENDENCY-LICENSES.txt`](packaging/DEPENDENCY-LICENSES.txt).
