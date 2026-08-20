namespace Quadrant.Core.Models;

public sealed record TaskItem(
    long Id,
    string Title,
    int QuadrantId,
    DateTimeOffset? DueAt,
    DateTimeOffset? ReminderAt,
    string? Note,
    bool IsCompleted,
    DateTimeOffset? CompletedAt,
    DateTimeOffset CreatedAt,
    DateTimeOffset UpdatedAt);
