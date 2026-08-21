using Quadrant.Core.Enums;

namespace Quadrant.Core.Models;

public sealed record FocusSessionStartRequest(
    long? TaskId,
    FocusMode Mode,
    PomodoroKind? PomodoroKind = null,
    DateTimeOffset? TargetEndAtUtc = null);
