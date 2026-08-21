using Quadrant.Core.Models;
using Quadrant.Core.Enums;

namespace Quadrant.Core.Interfaces;

public interface IRecurrenceService
{
    DateOnly GetNextOccurrence(DateOnly occurrenceDate, RecurrenceKind recurrenceKind, int recurrenceInterval, int monthlyAnchorDay);

    TaskDraft? BuildNextDraft(TaskItem task, DateTimeOffset now, TimeZoneInfo timeZone);
}
