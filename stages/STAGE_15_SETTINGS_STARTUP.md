# Stage 15 — Settings, Quadrant Naming, Theme, Startup

## Goal

加入少量真正必要设置，不做“设置中心”。

## Before coding — MUST browse

重新查：

- WPF `ThemeMode` current values/behavior；
- Windows startup apps current Microsoft guidance；
- HKCU Run key / Startup folder 对 unpackaged desktop app 的当前建议；
- Task Manager startup visibility/disable behavior。

若最新官方建议明显优于规划时 HKCU Run，记录并采用；如果需要 package identity 才能用，则继续用适合 unpackaged 的 user-level 方案。

## Settings fields

- Theme: System/Light/Dark
- Close behavior: MinimizeToTray/Exit
- LaunchAtStartup bool
- StartMinimized bool
- GlobalHotkey chord
- Quadrant Q1..Q4 Name + Subtitle

不要更多。

### Persistence

为了不再引 JSON 配置系统，可选择：

A. SQLite `settings` key/value + quadrants 表；或
B. 一个非常小的 JSON settings file。

**Preferred:** SQLite settings table，保持本地状态统一。Migration 002 增加 settings table，不破坏 v1 DB。

### Startup service

规划默认：HKCU `Software\Microsoft\Windows\CurrentVersion\Run`：

- value name `Quadrant`；
- quoted executable path；
- append `--background`；
- only current user；
- write/delete without admin。

`--background`：初始化 DB/notification/hotkey/tray，但不 show MainWindow。

若 exe 路径改变，下一次启用/设置保存刷新 Run value。

### Theme

设置变化即时应用 `ThemeMode`（按官方支持方式）；不要重建自定义 palette。

### Quadrant name

编辑只影响 display name/subtitle，ID 和 position 永远不变。

## Acceptance

- restart 后所有设置持久；
- light/dark/system；
- startup 开/关能在注册表/Windows Startup app 中观察；
- startup background 不抢前台；
- quadrant rename 不影响已有 task quadrant id；
- invalid hotkey 保存时反馈。

## DO NOT

- 不做 account/preferences sync；
- 不做 configurable colors V1；
- 不做 arbitrary fifth quadrant。

## Handoff

STATUS 写 migration 002 与 startup mechanism。下一 Stage 16。
