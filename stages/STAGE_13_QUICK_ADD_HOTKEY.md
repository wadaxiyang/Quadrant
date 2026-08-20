# Stage 13 — Quick Add Window and Global Hotkey

## Goal

实现从任意 Windows 工作场景快速记任务，是 V1 的核心低摩擦入口。

## Before coding — MUST browse

查最新 Win32：

- `RegisterHotKey`
- `UnregisterHotKey`
- `WM_HOTKEY`
- modifiers / `MOD_NOREPEAT`
- WPF HWND hook 官方/可靠 Microsoft 文档

## Implementation

### `GlobalHotkeyService`

通过 P/Invoke：

- RegisterHotKey
- UnregisterHotKey

WPF：`WindowInteropHelper` / `HwndSource.AddHook` 接 `WM_HOTKEY`。

默认：Ctrl + Alt + Q + MOD_NOREPEAT。

不要默认 Win-key；不要 F12。

### Lifecycle

- Main window HWND ready 后 register；
- setting change → unregister old then register new；
- app exit → unregister；
- conflict return false/GetLastError → UI 友好提示，app 继续运行。

### QuickAddWindow

设计遵守固定 Design System：

- Title autofocus；
- Q1..Q4；
- `Ctrl+1..4` 选择象限；
- Enter save；Esc close；
- “时间与提醒”折叠区域，可复用 Stage 06/09 的日期时间 ViewModel/control；
- 保存后窗口隐藏/关闭，焦点返回原 app。

### Window behavior

- top-level normal WPF Window；
- `ShowActivated=true`；
- 不永久 TopMost；
- 每次打开清空上次 draft；
- 窗口位置可居中当前工作区/主窗口，V1 不要求多屏智能记忆。

## Acceptance

- Word/Edge/VS Code 前台都能热键打开；
- 连按不会生成多个 QuickAddWindow；
- 热键冲突时不崩；
- 中文输入法正常；
- 完成保存后 task 立即出现对应 quadrant；
- app 主窗口隐藏在后台仍能 hotkey。

## DO NOT

- 不监听 keyboard hook；
- 不使用低级全局键盘抓取；
- 不做 natural language parser。

## Handoff

STATUS 写 P/Invoke signature 来源与默认 hotkey。下一 Stage 14。
