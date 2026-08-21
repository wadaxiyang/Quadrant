using Quadrant.Core.Enums;

namespace Quadrant.Core.Models;

public sealed record TaskUpdate(
    long Id,
    string Title,
    int? QuadrantId,
    DateTimeOffset? DueAt,
    DateTimeOffset? ReminderAt,
    string? Note,
    DateOnly? PlannedDate = null,
    int? EstimatedMinutes = null,
    RecurrenceKind RecurrenceKind = RecurrenceKind.None,
    int RecurrenceInterval = 1,
    string? RecurrenceSeriesId = null,
    int? RecurrenceAnchorDay = null);
