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

    private static IReadOnlyList<QuadrantDefinition> Quadrants { get; } =
    [
        new QuadrantDefinition(1, "Q1", ""), new QuadrantDefinition(2, "Q2", ""),
        new QuadrantDefinition(3, "Q3", ""), new QuadrantDefinition(4, "Q4", "")
    ];

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset Now { get; } = new(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8));
    }
}
