using Quadrant.App.ViewModels;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class TodayPageViewModelTests
{
    [Fact]
    public async Task Activate_loads_sections_and_deactivate_stops_change_refreshes()
    {
        var task = new TaskItem(1, "Today", 1, null, null, null, false, null, DateTimeOffset.UtcNow, DateTimeOffset.UtcNow);
        var query = new FakeQuery(new TodaySnapshot([], [task], [], [], 1, 90, 3_900));
        var hub = new AppChangeHub();
        var original = SynchronizationContext.Current;
        SynchronizationContext.SetSynchronizationContext(new ImmediateContext());
        try
        {
            using var viewModel = new TodayPageViewModel(query, hub);
            await viewModel.ActivateAsync();
            Assert.True(viewModel.HasPlannedToday);
            Assert.Equal("1 小时 30 分", viewModel.EstimatedTimeText);
            Assert.Equal("1 小时 5 分", viewModel.FocusedTimeText);
            var calls = query.CallCount;
            viewModel.Deactivate();
            hub.Publish(new AppChange(task.Id, AppChangeKind.TaskPlanned));
            Assert.Equal(calls, query.CallCount);
        }
        finally { SynchronizationContext.SetSynchronizationContext(original); }
    }


    [Fact]
    public async Task Completed_focus_session_refreshes_today_summary()
    {
        var query = new FakeQuery(new TodaySnapshot([], [], [], [], 0, 0, 600));
        var hub = new AppChangeHub();
        var original = SynchronizationContext.Current;
        SynchronizationContext.SetSynchronizationContext(new ImmediateContext());
        try
        {
            using var viewModel = new TodayPageViewModel(query, hub);
            await viewModel.ActivateAsync();
            var calls = query.CallCount;

            hub.Publish(new AppChange(0, AppChangeKind.FocusSessionCompleted));

            Assert.Equal(calls + 1, query.CallCount);
        }
        finally { SynchronizationContext.SetSynchronizationContext(original); }
    }

    private sealed class FakeQuery(TodaySnapshot snapshot) : ITodayQueryService
    { public int CallCount { get; private set; } public Task<TodaySnapshot> GetSnapshotAsync(CancellationToken cancellationToken = default) { CallCount++; return Task.FromResult(snapshot); } }
    private sealed class ImmediateContext : SynchronizationContext { public override void Post(SendOrPostCallback callback, object? state) => callback(state); }
}
