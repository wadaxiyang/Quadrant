using System.Reflection;
using Quadrant.App.ViewModels;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class FocusPageViewModelTests
{
    [Fact]
    public void Idle_modes_use_configured_pomodoro_and_zero_stopwatch_time()
    {
        var day = new DateOnly(2026, 8, 22);
        var task = new TaskItem(1, "Plan", 2, null, null, null, false, null, DateTimeOffset.UnixEpoch, DateTimeOffset.UnixEpoch, day, 45);
        var viewModel = CreateViewModel([task], day, new PomodoroSettings(FocusMinutes: 40));

        Assert.Equal("40:00", viewModel.TimerText);
        Assert.Equal("Q2 · Today · 预计 45 分钟", viewModel.TaskOptions[0].Metadata);
        Assert.True(viewModel.IsPomodoroMode);

        viewModel.Mode = FocusMode.Stopwatch;

        Assert.Equal("00:00", viewModel.TimerText);
        Assert.True(viewModel.IsStopwatchMode);
        Assert.True(viewModel.CanConfigureSession);
    }

    [Fact]
    public void Task_picker_filters_inbox_today_and_quadrants_and_selects_for_focus()
    {
        var day = new DateOnly(2026, 8, 22);
        var inbox = new TaskItem(1, "Capture", null, null, null, null, false, null, DateTimeOffset.UnixEpoch, DateTimeOffset.UnixEpoch);
        var q1 = new TaskItem(2, "Urgent", 1, null, null, null, false, null, DateTimeOffset.UnixEpoch, DateTimeOffset.UnixEpoch);
        var q2 = new TaskItem(3, "Plan", 2, null, null, null, false, null, DateTimeOffset.UnixEpoch, DateTimeOffset.UnixEpoch, day);
        var viewModel = CreateViewModel([inbox, q1, q2], day, new PomodoroSettings(), todayTaskIds: [1, 3]);

        Assert.Equal(["Capture", "Plan"], viewModel.FilteredTaskOptions.Select(option => option.Title));

        viewModel.SelectedTaskSource = viewModel.TaskSources.Single(source => source.Source == FocusTaskSource.Inbox);
        Assert.Equal(["Capture"], viewModel.FilteredTaskOptions.Select(option => option.Title));

        viewModel.SelectTask(inbox.Id, revealSource: false);
        Assert.Equal(inbox.Id, viewModel.SelectedTask?.Id);
        Assert.Equal("Inbox", viewModel.SelectedTaskOption!.Metadata);

        viewModel.SelectedTaskSource = viewModel.TaskSources.Single(source => source.Source == FocusTaskSource.Quadrant1);
        Assert.Equal(["Urgent"], viewModel.FilteredTaskOptions.Select(option => option.Title));
    }

    [Fact]
    public async Task Activation_loads_bounded_today_focus_summary()
    {
        var day = new DateOnly(2026, 8, 22);
        var sessions = DispatchProxy.Create<IFocusSessionService, FocusSessionServiceProxy>();
        ((FocusSessionServiceProxy)(object)sessions).Summary = new FocusDaySummary(4_500, 3);
        var viewModel = CreateViewModel([], day, new PomodoroSettings(), sessions);

        await viewModel.ActivateAsync();

        Assert.Equal("1 小时 15 分 · 3 次专注", viewModel.TodaySummaryText);
        Assert.Equal("25:00", viewModel.TimerText);
        Assert.False(viewModel.HasError);
    }

    private static FocusPageViewModel CreateViewModel(
        IReadOnlyList<TaskItem> tasks,
        DateOnly day,
        PomodoroSettings settings,
        IFocusSessionService? sessions = null,
        IEnumerable<long>? todayTaskIds = null)
    {
        sessions ??= DispatchProxy.Create<IFocusSessionService, FocusSessionServiceProxy>();
        var stopwatch = DispatchProxy.Create<IFocusTimerService, FocusTimerServiceProxy>();
        var clock = new FixedClock(day);
        var scheduler = DispatchProxy.Create<IFocusCompletionScheduler, ThrowingProxy>();
        return new FocusPageViewModel(tasks, stopwatch, new PomodoroTimerService(sessions, clock, scheduler), sessions, settings, day, todayTaskIds);
    }

    private class FocusSessionServiceProxy : DispatchProxy
    {
        public FocusDaySummary Summary { get; set; } = FocusDaySummary.Empty;

        protected override object? Invoke(MethodInfo? targetMethod, object?[]? args) => targetMethod?.Name switch
        {
            nameof(IFocusSessionService.GetCurrentAsync) => Task.FromResult<FocusSession?>(null),
            nameof(IFocusSessionService.GetProductiveSummaryAsync) => Task.FromResult(Summary),
            _ => throw new NotSupportedException(targetMethod?.Name)
        };
    }

    private class FocusTimerServiceProxy : DispatchProxy
    {
        protected override object? Invoke(MethodInfo? targetMethod, object?[]? args) => targetMethod?.Name switch
        {
            nameof(IFocusTimerService.GetSnapshot) => null,
            nameof(IFocusTimerService.RestoreAsync) => Task.FromResult<FocusTimerSnapshot?>(null),
            _ => throw new NotSupportedException(targetMethod?.Name)
        };
    }

    private class ThrowingProxy : DispatchProxy
    {
        protected override object? Invoke(MethodInfo? targetMethod, object?[]? args) =>
            throw new NotSupportedException(targetMethod?.Name);
    }

    private sealed class FixedClock(DateOnly day) : IClock
    {
        public DateTimeOffset UtcNow { get; } = new(day.Year, day.Month, day.Day, 8, 0, 0, TimeSpan.Zero);
        public DateTimeOffset LocalNow => UtcNow;
        public DateOnly LocalDate => day;
        public TimeZoneInfo LocalTimeZone => TimeZoneInfo.Utc;
        public long GetTimestamp() => 0;
        public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }
}
