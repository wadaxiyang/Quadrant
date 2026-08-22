using Quadrant.Core.Enums;
using Quadrant.Core.Services;

namespace Quadrant.Core.Models;

public sealed record AppSettings(
    string Theme,
    bool CloseToTray,
    bool LaunchAtStartup,
    bool StartMinimized,
    string GlobalHotkey,
    int? QuickCaptureQuadrantId = null,
    ReminderPreset DefaultReminder = ReminderPreset.None,
    int FocusMinutes = 25,
    int ShortBreakMinutes = 5,
    int LongBreakMinutes = 15,
    int LongBreakInterval = 4,
    bool AutoStartBreak = false,
    bool AutoStartFocus = false,
    bool TaskRemindersEnabled = true,
    bool FocusNotificationsEnabled = true,
    bool NotificationSoundEnabled = true,
    ReviewRange ReviewDefaultRange = ReviewRange.SevenDays,
    DayOfWeek WeekStart = DayOfWeek.Monday,
    double SidebarIconSize = 24)
{
    public static AppSettings Default { get; } = new("System", true, false, false, "Ctrl+Alt+Q");

    public PomodoroSettings Pomodoro => new(
        FocusMinutes,
        ShortBreakMinutes,
        LongBreakMinutes,
        LongBreakInterval,
        AutoStartBreak,
        AutoStartFocus);

    public void Validate()
    {
        if (Theme is not ("System" or "Light" or "Dark"))
            throw new TaskValidationException("Theme setting is invalid.");
        if (!string.Equals(GlobalHotkey.Trim(), "Ctrl+Alt+Q", StringComparison.OrdinalIgnoreCase))
            throw new TaskValidationException("Global hotkey setting is invalid.");
        if (QuickCaptureQuadrantId is < 1 or > 4)
            throw new TaskValidationException("Quick Capture destination is invalid.");
        if (!Enum.IsDefined(DefaultReminder) || !Enum.IsDefined(ReviewDefaultRange) || !Enum.IsDefined(WeekStart))
            throw new TaskValidationException("An enum setting is invalid.");
        if (SidebarIconSize is < 16 or > 32)
            throw new TaskValidationException("Sidebar icon size must be between 16 and 32.");
        Pomodoro.Validate();
    }
}
