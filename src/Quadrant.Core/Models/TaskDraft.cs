using Quadrant.Core.Enums;

namespace Quadrant.Core.Models;

public sealed record TaskDraft(
    string Title,
    int? QuadrantId,
    DateTimeOffset? DueAt = null,
    DateTimeOffset? ReminderAt = null,
    string? Note = null,
    DateOnly? PlannedDate = null,
    int? EstimatedMinutes = null,
    RecurrenceKind RecurrenceKind = RecurrenceKind.None,
    int RecurrenceInterval = 1,
    string? RecurrenceSeriesId = null,
    int? RecurrenceAnchorDay = null);
