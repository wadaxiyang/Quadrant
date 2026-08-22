using System.Reflection;
using Quadrant.App.ViewModels;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class MainViewModelInteractionTests
{
    [Fact]
    public void Local_add_requests_only_an_existing_quadrant()
    {
        var clock = new FixedClock();
        var focusSessions = Proxy<IFocusSessionService>();
        var changeHub = new AppChangeHub();
        var viewModel = new MainViewModel(
            Proxy<ITaskService>(),
            Proxy<IQuadrantRepository>(),
            clock,
            changeHub,
            Proxy<ITodayQueryService>(),
            Proxy<IFocusTimerService>(),
            new PomodoroTimerService(focusSessions, clock, Proxy<IFocusCompletionScheduler>()),
            focusSessions);
        viewModel.UpdateDefinitions(Quadrants);

        var requested = new List<int>();
        viewModel.NewTaskInQuadrantRequested += (_, args) => requested.Add(args.QuadrantId);

        viewModel.NewTaskInQuadrantCommand.Execute(2);
        viewModel.NewTaskInQuadrantCommand.Execute(5);

        Assert.Equal([2], requested);
    }

    [Fact]
    public async Task Quadrant_move_can_be_conditionally_restored()
    {
        var taskService = DispatchProxy.Create<ITaskService, StatefulTaskServiceProxy>();
        var taskServiceProxy = (StatefulTaskServiceProxy)(object)taskService;
        taskServiceProxy.Current = CreateTask(1);
        var viewModel = CreateViewModel(taskService);

        var moved = await viewModel.MoveTaskAsync(new MoveTaskRequest(7, 2));
        var restored = await viewModel.RestoreMovedTaskAsync(7, 2, 1);

        Assert.Equal(2, moved?.QuadrantId);
        Assert.Equal(1, restored?.QuadrantId);
        Assert.Equal(2, taskServiceProxy.MoveCount);
    }

    [Fact]
    public async Task Quadrant_move_undo_does_not_overwrite_a_later_move()
    {
        var taskService = DispatchProxy.Create<ITaskService, StatefulTaskServiceProxy>();
        var taskServiceProxy = (StatefulTaskServiceProxy)(object)taskService;
        taskServiceProxy.Current = CreateTask(3);
        var viewModel = CreateViewModel(taskService);

        var restored = await viewModel.RestoreMovedTaskAsync(7, 2, 1);

        Assert.Null(restored);
        Assert.Equal(3, taskServiceProxy.Current.QuadrantId);
        Assert.Equal(0, taskServiceProxy.MoveCount);
    }

    private static MainViewModel CreateViewModel(ITaskService taskService)
    {
        var clock = new FixedClock();
        var focusSessions = Proxy<IFocusSessionService>();
        var viewModel = new MainViewModel(
            taskService,
            Proxy<IQuadrantRepository>(),
            clock,
            new AppChangeHub(),
            Proxy<ITodayQueryService>(),
            Proxy<IFocusTimerService>(),
            new PomodoroTimerService(focusSessions, clock, Proxy<IFocusCompletionScheduler>()),
            focusSessions);
        viewModel.UpdateDefinitions(Quadrants);
        return viewModel;
    }

    private static TaskItem CreateTask(int quadrantId) => new(
        7,
        "Task",
        quadrantId,
        null,
        null,
        null,
        false,
        null,
        DateTimeOffset.UnixEpoch,
        DateTimeOffset.UnixEpoch);

    private static T Proxy<T>() where T : class => DispatchProxy.Create<T, ThrowingProxy>();

    private static IReadOnlyList<QuadrantDefinition> Quadrants { get; } =
    [
        new(1, "Q1", ""), new(2, "Q2", ""), new(3, "Q3", ""), new(4, "Q4", "")
    ];

    private class ThrowingProxy : DispatchProxy
    {
        protected override object? Invoke(MethodInfo? targetMethod, object?[]? args) =>
            throw new NotSupportedException(targetMethod?.Name);
    }

    private class StatefulTaskServiceProxy : DispatchProxy
    {
        public TaskItem Current { get; set; } = null!;
        public int MoveCount { get; private set; }

        protected override object? Invoke(MethodInfo? targetMethod, object?[]? args)
        {
            if (targetMethod?.Name == nameof(ITaskService.GetByIdAsync))
            {
                var taskId = (long)args![0]!;
                return Task.FromResult<TaskItem?>(Current.Id == taskId ? Current : null);
            }

            if (targetMethod?.Name == nameof(ITaskService.MoveTaskAsync))
            {
                var taskId = (long)args![0]!;
                var targetQuadrantId = (int)args[1]!;
                if (Current.Id != taskId)
                {
                    return Task.FromResult<TaskItem?>(null);
                }

                MoveCount++;
                Current = Current with { QuadrantId = targetQuadrantId };
                return Task.FromResult<TaskItem?>(Current);
            }

            throw new NotSupportedException(targetMethod?.Name);
        }
    }

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset LocalNow { get; } = new(2026, 8, 22, 9, 0, 0, TimeSpan.FromHours(8));
        public DateTimeOffset UtcNow => LocalNow.ToUniversalTime();
        public DateOnly LocalDate => DateOnly.FromDateTime(LocalNow.Date);
        public TimeZoneInfo LocalTimeZone => TimeZoneInfo.CreateCustomTimeZone("Test", LocalNow.Offset, "Test", "Test");
        public long GetTimestamp() => 0;
        public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }
}
