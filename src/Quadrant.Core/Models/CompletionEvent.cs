namespace Quadrant.Core.Models;

public sealed record CompletionEvent(
    string Id, long? TaskId, DateTimeOffset CompletedAtUtc, DateOnly CompletedLocalDate,
    int? QuadrantSnapshot, string TaskTitleSnapshot, DateTimeOffset? DueAtUtcSnapshot,
    DateOnly? PlannedDateSnapshot, int? EstimatedMinutesSnapshot, bool WasOverdue,
    DateTimeOffset? RevertedAtUtc = null);
