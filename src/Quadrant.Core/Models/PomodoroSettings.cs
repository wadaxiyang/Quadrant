namespace Quadrant.Core.Models;

public sealed record PomodoroSettings(
    int FocusMinutes = 25,
    int ShortBreakMinutes = 5,
    int LongBreakMinutes = 15,
    int LongBreakInterval = 4,
    bool AutoStartBreak = false,
    bool AutoStartFocus = false)
{
    public void Validate()
    {
        if (FocusMinutes is < 1 or > 240 || ShortBreakMinutes is < 1 or > 120 || LongBreakMinutes is < 1 or > 120 || LongBreakInterval is < 2 or > 12)
            throw new Services.TaskValidationException("Pomodoro settings are invalid.");
    }
}
