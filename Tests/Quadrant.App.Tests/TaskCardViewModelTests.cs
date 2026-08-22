using System.Windows.Input;
using Quadrant.App.ViewModels;
using Quadrant.Core.Enums;
using Quadrant.Core.Models;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class TaskCardViewModelTests
{
    [Fact]
    public void Constructor_BuildsCompactMetadataAndOverdueState()
    {
        var now = new DateTimeOffset(2026, 8, 22, 10, 0, 0, TimeSpan.Zero);
        var command = new NoOpCommand();
        var task = new TaskItem(
            7,
            "Prepare launch",
            1,
            now.AddHours(-1),
            now.AddHours(1),
            null,
            false,
            null,
            now,
            now,
            DateOnly.FromDateTime(now.Date),
            45,
            RecurrenceKind.Weekly);

        var viewModel = new TaskCardViewModel(task, command, command, command, command, command, command, command, now, TimeZoneInfo.Utc);

        Assert.True(viewModel.HasDue);
        Assert.True(viewModel.HasMetadata);
        Assert.True(viewModel.IsOverdue);
        Assert.True(viewModel.IsPlannedForToday);
        Assert.Equal("今天 09:00", viewModel.DueMetadataText);
        Assert.Equal("提醒 今天 11:00", viewModel.ReminderMetadataText);
        Assert.Equal("Today", viewModel.PlanMetadataText);
        Assert.Equal("45 分钟", viewModel.EstimateMetadataText);
        Assert.Equal("每周", viewModel.RecurrenceMetadataText);
        Assert.Contains("已逾期", viewModel.AutomationName);
        Assert.Same(command, viewModel.FocusCommand);
    }

    [Fact]
    public void Constructor_HidesMetadataWhenTaskHasNone()
    {
        var now = new DateTimeOffset(2026, 8, 22, 10, 0, 0, TimeSpan.Zero);
        var command = new NoOpCommand();
        var task = new TaskItem(8, "Plain task", 1, null, null, null, false, null, now, now);

        var viewModel = new TaskCardViewModel(task, command, command, command, command, command, command, command, now, TimeZoneInfo.Utc);

        Assert.False(viewModel.HasDue);
        Assert.False(viewModel.HasMetadata);
        Assert.False(viewModel.IsOverdue);
        Assert.Equal("Plain task", viewModel.AutomationName);
    }

    private sealed class NoOpCommand : ICommand
    {
        public event EventHandler? CanExecuteChanged
        {
            add { }
            remove { }
        }

        public bool CanExecute(object? parameter) => true;

        public void Execute(object? parameter)
        {
        }
    }
}
