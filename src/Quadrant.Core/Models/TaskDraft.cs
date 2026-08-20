namespace Quadrant.Core.Models;

public sealed record TaskDraft(
    string Title,
    int QuadrantId,
    DateTimeOffset? DueAt = null,
    DateTimeOffset? ReminderAt = null,
    string? Note = null);
