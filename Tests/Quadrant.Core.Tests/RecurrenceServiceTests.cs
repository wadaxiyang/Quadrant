using Quadrant.Core.Enums;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class RecurrenceServiceTests
{
    private readonly RecurrenceService service = new();

    [Theory]
    [InlineData(RecurrenceKind.Daily, 2026, 12, 31, 2027, 1, 1)]
    [InlineData(RecurrenceKind.Weekly, 2026, 12, 28, 2027, 1, 4)]
    public void Daily_and_weekly_cross_calendar_boundaries(RecurrenceKind kind, int year, int month, int day, int expectedYear, int expectedMonth, int expectedDay)
    {
        var next = service.GetNextOccurrence(new DateOnly(year, month, day), kind, 1, day);

        Assert.Equal(new DateOnly(expectedYear, expectedMonth, expectedDay), next);
    }

    [Theory]
    [InlineData(2026, 1, 31, 2026, 2, 28)]
    [InlineData(2028, 1, 31, 2028, 2, 29)]
    [InlineData(2026, 4, 30, 2026, 5, 31)]
    public void Monthly_uses_stable_anchor_and_clamps_short_months(int year, int month, int day, int expectedYear, int expectedMonth, int expectedDay)
    {
        var next = service.GetNextOccurrence(new DateOnly(year, month, day), RecurrenceKind.Monthly, 1, 31);

        Assert.Equal(new DateOnly(expectedYear, expectedMonth, expectedDay), next);
    }

    [Fact]
    public void Monthly_after_clamp_returns_to_anchor_day()
    {
        var next = service.GetNextOccurrence(new DateOnly(2026, 2, 28), RecurrenceKind.Monthly, 1, 31);

        Assert.Equal(new DateOnly(2026, 3, 31), next);
    }

    [Fact]
    public void Build_next_draft_moves_due_planned_and_reminder_preserving_due_offset()
    {
        var zone = TimeZoneInfo.CreateCustomTimeZone("Test", TimeSpan.FromHours(8), "Test", "Test");
        var due = new DateTimeOffset(2026, 1, 31, 9, 0, 0, TimeSpan.FromHours(8));
        var source = Task(due, due.AddHours(-2), new DateOnly(2026, 1, 31), RecurrenceKind.Monthly, null, null);

        var next = service.BuildNextDraft(source, due, zone)!;

        Assert.Equal(new DateTimeOffset(2026, 2, 28, 9, 0, 0, TimeSpan.FromHours(8)), next.DueAt);
        Assert.Equal(next.DueAt!.Value.AddHours(-2), next.ReminderAt);
        Assert.Equal(new DateOnly(2026, 2, 28), next.PlannedDate);
        Assert.Equal(31, next.RecurrenceAnchorDay);
        Assert.NotNull(next.RecurrenceSeriesId);
    }

    [Fact]
    public void Reminder_only_uses_its_own_wall_time()
    {
        var zone = TimeZoneInfo.CreateCustomTimeZone("Test", TimeSpan.FromHours(8), "Test", "Test");
        var reminder = new DateTimeOffset(2026, 8, 20, 7, 15, 0, TimeSpan.FromHours(8));
        var source = Task(null, reminder, null, RecurrenceKind.Weekly, "series", null);

        var next = service.BuildNextDraft(source, reminder, zone)!;

        Assert.Null(next.DueAt);
        Assert.Equal(new DateTimeOffset(2026, 8, 27, 7, 15, 0, TimeSpan.FromHours(8)), next.ReminderAt);
        Assert.Equal("series", next.RecurrenceSeriesId);
    }

    [Fact]
    public void Dst_invalid_time_moves_forward_and_ambiguous_time_chooses_earlier_instant()
    {
        var zone = CreateDstZone();
        var spring = Task(new DateTimeOffset(2026, 3, 1, 2, 30, 0, TimeSpan.FromHours(-8)), null, null, RecurrenceKind.Weekly, "series", null);
        var fall = Task(new DateTimeOffset(2026, 10, 25, 1, 30, 0, TimeSpan.FromHours(-7)), null, null, RecurrenceKind.Weekly, "series", null);

        var nextSpring = service.BuildNextDraft(spring, spring.DueAt!.Value, zone)!;
        var nextFall = service.BuildNextDraft(fall, fall.DueAt!.Value, zone)!;

        Assert.Equal(new DateTime(2026, 3, 8, 3, 0, 0), TimeZoneInfo.ConvertTime(nextSpring.DueAt!.Value, zone).DateTime);
        Assert.Equal(new DateTimeOffset(2026, 11, 1, 1, 30, 0, TimeSpan.FromHours(-7)), nextFall.DueAt);
    }

    [Fact]
    public void Unsupported_interval_is_rejected()
    {
        Assert.Throws<TaskValidationException>(() => service.GetNextOccurrence(new DateOnly(2026, 8, 1), RecurrenceKind.Daily, 2, 1));
    }

    private static TaskItem Task(DateTimeOffset? due, DateTimeOffset? reminder, DateOnly? planned, RecurrenceKind kind, string? series, int? anchor) =>
        new(1, "Recurring", null, due, reminder, "note", false, null, DateTimeOffset.UtcNow, DateTimeOffset.UtcNow, planned, 30, kind, 1, series, anchor);

    private static TimeZoneInfo CreateDstZone()
    {
        var start = TimeZoneInfo.TransitionTime.CreateFloatingDateRule(new DateTime(1, 1, 1, 2, 0, 0), 3, 2, DayOfWeek.Sunday);
        var end = TimeZoneInfo.TransitionTime.CreateFloatingDateRule(new DateTime(1, 1, 1, 2, 0, 0), 11, 1, DayOfWeek.Sunday);
        var rule = TimeZoneInfo.AdjustmentRule.CreateAdjustmentRule(new DateTime(2020, 1, 1), new DateTime(2030, 12, 31), TimeSpan.FromHours(1), start, end);
        return TimeZoneInfo.CreateCustomTimeZone("Dst", TimeSpan.FromHours(-8), "Dst", "Standard", "Daylight", [rule]);
    }
}
