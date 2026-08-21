namespace Quadrant.Core.Models;

public sealed record TodaySnapshot(
    IReadOnlyList<TaskItem> Overdue,
    IReadOnlyList<TaskItem> PlannedToday,
    IReadOnlyList<TaskItem> DueToday,
    IReadOnlyList<TaskItem> NeedsReschedule,
    int UniqueTaskCount,
    long EstimatedMinutesTotal,
    long FocusedSecondsToday)
{
    public static TodaySnapshot Empty { get; } = new([], [], [], [], 0, 0, 0);
}
