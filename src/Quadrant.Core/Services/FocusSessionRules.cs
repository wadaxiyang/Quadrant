using Quadrant.Core.Enums;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public static class FocusSessionRules
{
    public static bool IsProductive(FocusSession session) =>
        session.Status == FocusStatus.Completed &&
        (session.Mode == FocusMode.Stopwatch || session.PomodoroKind == PomodoroKind.Focus);

    public static void ValidateStart(FocusSessionStartRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);
        if (!Enum.IsDefined(request.Mode) || (request.Mode == FocusMode.Pomodoro && request.PomodoroKind is null) ||
            (request.Mode == FocusMode.Stopwatch && request.PomodoroKind is not null) ||
            (request.PomodoroKind is { } kind && !Enum.IsDefined(kind)) ||
            (request.PomodoroKind is PomodoroKind.ShortBreak or PomodoroKind.LongBreak && request.TaskId is not null))
        {
            throw new TaskValidationException("Focus session configuration is invalid.");
        }
    }

    public static void ValidateDuration(int durationSeconds)
    {
        if (durationSeconds < 0)
        {
            throw new TaskValidationException("Focus duration cannot be negative.");
        }
    }
}
