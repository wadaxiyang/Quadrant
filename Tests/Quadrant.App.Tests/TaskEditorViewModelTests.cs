using Quadrant.App.ViewModels;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class TaskEditorViewModelTests
{
    [Fact]
    public void Quick_capture_defaults_to_inbox_and_can_explicitly_select_a_quadrant()
    {
        var viewModel = new TaskEditorViewModel(Quadrants, new FixedClock(), allowInbox: true) { Title = "收集任务" };

        Assert.Null(viewModel.QuadrantId);
        Assert.Equal("Inbox（未分类）", viewModel.QuadrantLabel);
        Assert.True(viewModel.TryBuildDraft(out var inboxDraft));
        Assert.Null(inboxDraft.QuadrantId);
        Assert.Null(inboxDraft.DueAt);
        Assert.Null(inboxDraft.ReminderAt);

        viewModel.QuadrantId = 2;
        Assert.True(viewModel.TryBuildDraft(out var quadrantDraft));
        Assert.Equal(2, quadrantDraft.QuadrantId);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(5)]
    public void Quick_capture_rejects_invalid_quadrant(int quadrantId)
    {
        var viewModel = new TaskEditorViewModel(Quadrants, new FixedClock(), allowInbox: true)
        {
            Title = "任务",
            QuadrantId = quadrantId
        };

        Assert.False(viewModel.TryBuildDraft(out _));
        Assert.Equal("请选择有效象限。", viewModel.TitleError);
    }

    [Fact]
    public void Standard_editor_keeps_a_required_quadrant()
    {
        var viewModel = new TaskEditorViewModel(Quadrants, new FixedClock()) { Title = "任务", QuadrantId = null };

        Assert.False(viewModel.TryBuildDraft(out _));
        Assert.Equal("请选择象限。", viewModel.TitleError);
    }

    [Fact]
    public void Editor_round_trips_planned_date_and_estimate_without_deriving_from_due()
    {
        var viewModel = new TaskEditorViewModel(Quadrants, new FixedClock())
        {
            Title = "计划任务",
            QuadrantId = 1,
            DueDate = new DateTime(2026, 8, 25),
            PlannedDate = new DateTime(2026, 8, 22),
            EstimatedMinutesText = "90"
        };

        Assert.True(viewModel.TryBuildDraft(out var draft));
        Assert.Equal(new DateOnly(2026, 8, 22), draft.PlannedDate);
        Assert.Equal(90, draft.EstimatedMinutes);
    }

    [Fact]
    public void Editor_update_preserves_plan_estimate_and_existing_recurrence_metadata()
    {
        var task = new TaskItem(12, "原任务", 2, null, null, null, false, null,
            new DateTimeOffset(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8)),
            new DateTimeOffset(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8)),
            new DateOnly(2026, 8, 23), 30, Quadrant.Core.Enums.RecurrenceKind.Monthly, 1, "series", 31);
        var viewModel = new TaskEditorViewModel(Quadrants, new FixedClock(), task)
        {
            Title = "更新后"
        };

        Assert.True(viewModel.TryBuildUpdate(out var update));
        Assert.Equal(new DateOnly(2026, 8, 23), update.PlannedDate);
        Assert.Equal(30, update.EstimatedMinutes);
        Assert.Equal(Quadrant.Core.Enums.RecurrenceKind.Monthly, update.RecurrenceKind);
        Assert.Equal("series", update.RecurrenceSeriesId);
        Assert.Equal(31, update.RecurrenceAnchorDay);
    }

    [Theory]
    [InlineData("0")]
    [InlineData("1441")]
    [InlineData("1.5")]
    public void Editor_rejects_invalid_estimate(string value)
    {
        var viewModel = new TaskEditorViewModel(Quadrants, new FixedClock())
        {
            Title = "任务",
            QuadrantId = 1,
            EstimatedMinutesText = value
        };

        Assert.False(viewModel.TryBuildDraft(out _));
        Assert.Equal("预计时长需为 1–1440 分钟的整数。", viewModel.PlanningError);
    }

    private static IReadOnlyList<QuadrantDefinition> Quadrants { get; } =
    [
        new QuadrantDefinition(1, "Q1", ""), new QuadrantDefinition(2, "Q2", ""),
        new QuadrantDefinition(3, "Q3", ""), new QuadrantDefinition(4, "Q4", "")
    ];

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset LocalNow { get; } = new(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8));
        public DateTimeOffset UtcNow => LocalNow.ToUniversalTime();
        public DateOnly LocalDate => DateOnly.FromDateTime(LocalNow.Date);
        public TimeZoneInfo LocalTimeZone => TimeZoneInfo.CreateCustomTimeZone("Test", LocalNow.Offset, "Test", "Test");
        public long GetTimestamp() => 0;
        public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }
}
