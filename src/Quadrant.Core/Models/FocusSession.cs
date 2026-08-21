using Quadrant.Core.Enums;

namespace Quadrant.Core.Models;

public sealed record FocusSession(
    string Id, long? TaskId, FocusMode Mode, DateTimeOffset StartedAtUtc,
    DateTimeOffset? ActiveSegmentStartedAtUtc, DateTimeOffset? EndedAtUtc,
    DateTimeOffset? TargetEndAtUtc, int DurationSeconds, FocusStatus Status,
    PomodoroKind? PomodoroKind, DateOnly CreatedLocalDate, string? TaskTitleSnapshot,
    int? QuadrantSnapshot);
