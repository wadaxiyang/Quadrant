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
