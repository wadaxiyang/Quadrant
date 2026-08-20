namespace Quadrant.Core.Models;

public sealed record TaskUpdate(
    long Id,
    string Title,
    int QuadrantId,
    DateTimeOffset? DueAt,
    DateTimeOffset? ReminderAt,
    string? Note);
