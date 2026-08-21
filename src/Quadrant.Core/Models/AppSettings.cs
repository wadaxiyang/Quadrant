namespace Quadrant.Core.Models;

public sealed record AppSettings(
    string Theme,
    bool CloseToTray,
    bool LaunchAtStartup,
    bool StartMinimized,
    string GlobalHotkey);
