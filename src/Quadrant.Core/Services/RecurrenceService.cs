using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class RecurrenceService : IRecurrenceService
{
    public DateOnly GetNextOccurrence(DateOnly occurrenceDate, RecurrenceKind recurrenceKind, int recurrenceInterval, int monthlyAnchorDay)
    {
        ValidateRule(recurrenceKind, recurrenceInterval, monthlyAnchorDay);
        return recurrenceKind switch
        {
            RecurrenceKind.Daily => occurrenceDate.AddDays(1),
            RecurrenceKind.Weekly => occurrenceDate.AddDays(7),
            RecurrenceKind.Monthly => AddMonth(occurrenceDate, monthlyAnchorDay),
            _ => throw new TaskValidationException("A recurrence rule is required.")
        };
    }

    public TaskDraft? BuildNextDraft(TaskItem task, DateTimeOffset now, TimeZoneInfo timeZone)
    {
        ArgumentNullException.ThrowIfNull(task);
        ArgumentNullException.ThrowIfNull(timeZone);
        if (task.RecurrenceKind == RecurrenceKind.None)
        {
            return null;
        }

        var localToday = DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(now, timeZone).Date);
        var anchorDay = task.RecurrenceAnchorDay
            ?? (task.DueAt is { } due ? LocalDate(due, timeZone).Day : task.PlannedDate?.Day ?? localToday.Day);
        ValidateRule(task.RecurrenceKind, task.RecurrenceInterval, anchorDay);

        DateTimeOffset? nextDue = task.DueAt is { } dueAt
            ? MoveInstant(dueAt, task.RecurrenceKind, anchorDay, timeZone)
            : null;
        DateTimeOffset? nextReminder = task.ReminderAt is { } reminderAt
            ? task.DueAt is { } originalDue
                ? nextDue!.Value.Add(reminderAt - originalDue)
                : MoveInstant(reminderAt, task.RecurrenceKind, anchorDay, timeZone)
            : null;
        DateOnly? nextPlanned = task.PlannedDate is { } plannedDate
            ? GetNextOccurrence(plannedDate, task.RecurrenceKind, task.RecurrenceInterval, anchorDay)
            : null;

        return new TaskDraft(task.Title, task.QuadrantId, nextDue, nextReminder, task.Note, nextPlanned,
            task.EstimatedMinutes, task.RecurrenceKind, 1, task.RecurrenceSeriesId ?? Guid.NewGuid().ToString("N"), anchorDay);
    }

    private static void ValidateRule(RecurrenceKind recurrenceKind, int recurrenceInterval, int monthlyAnchorDay)
    {
        if (!Enum.IsDefined(recurrenceKind) || recurrenceKind == RecurrenceKind.None || recurrenceInterval != 1 || monthlyAnchorDay is < 1 or > 31)
        {
            throw new TaskValidationException("Only daily, weekly, or monthly recurrence with an interval of 1 is supported.");
        }
    }

    private static DateOnly AddMonth(DateOnly date, int anchorDay)
    {
        var target = date.AddMonths(1);
        return new DateOnly(target.Year, target.Month, Math.Min(anchorDay, DateTime.DaysInMonth(target.Year, target.Month)));
    }

    private DateTimeOffset MoveInstant(DateTimeOffset value, RecurrenceKind kind, int anchorDay, TimeZoneInfo timeZone)
    {
        var local = TimeZoneInfo.ConvertTime(value, timeZone);
        var targetDate = GetNextOccurrence(DateOnly.FromDateTime(local.Date), kind, 1, anchorDay);
        return ResolveAutomatic(targetDate.ToDateTime(TimeOnly.FromTimeSpan(local.TimeOfDay)), timeZone);
    }

    private static DateOnly LocalDate(DateTimeOffset value, TimeZoneInfo timeZone) =>
        DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(value, timeZone).Date);

    private static DateTimeOffset ResolveAutomatic(DateTime localTime, TimeZoneInfo timeZone)
    {
        var unspecified = DateTime.SpecifyKind(localTime, DateTimeKind.Unspecified);
        // A skipped spring-forward wall time rolls forward to the first valid minute;
        // a repeated fall-back time uses the earlier instant (the largest UTC offset).
        while (timeZone.IsInvalidTime(unspecified))
        {
            unspecified = unspecified.AddMinutes(1);
        }

        var offset = timeZone.IsAmbiguousTime(unspecified)
            ? timeZone.GetAmbiguousTimeOffsets(unspecified).Max()
            : timeZone.GetUtcOffset(unspecified);
        return new DateTimeOffset(unspecified, offset);
    }
}
