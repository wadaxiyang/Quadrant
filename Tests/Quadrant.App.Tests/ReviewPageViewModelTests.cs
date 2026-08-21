using Quadrant.App.ViewModels;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class ReviewPageViewModelTests
{
    [Fact]
    public async Task Activating_loads_formatting_and_range_change_reloads()
    {
        var query = new FakeQuery(); var hub = new AppChangeHub();
        var original = SynchronizationContext.Current; SynchronizationContext.SetSynchronizationContext(new ImmediateContext());
        try
        {
            using var viewModel = new ReviewPageViewModel(query, hub);
            await viewModel.ActivateAsync();
            Assert.Equal("1 小时 1 分", viewModel.FocusTimeText);
            Assert.Equal("30 分", viewModel.AverageFocusText);
            Assert.Equal("3", viewModel.CurrentInboxText);
            viewModel.SelectedRange = ReviewRange.ThirtyDays;
            await WaitForAsync(() => query.Ranges.Contains(ReviewRange.ThirtyDays));
            Assert.Equal(2, query.SummaryCalls);
        }
        finally { SynchronizationContext.SetSynchronizationContext(original); }
    }

    [Fact]
    public async Task Deactivate_unsubscribes_and_error_can_retry()
    {
        var query = new FakeQuery { FailNext = true }; var hub = new AppChangeHub();
        var original = SynchronizationContext.Current; SynchronizationContext.SetSynchronizationContext(new ImmediateContext());
        try
        {
            using var viewModel = new ReviewPageViewModel(query, hub);
            await viewModel.ActivateAsync();
            Assert.True(viewModel.HasError);
            await viewModel.LoadAsync();
            Assert.False(viewModel.HasError);
            var calls = query.SummaryCalls;
            viewModel.Deactivate(); hub.Publish(new AppChange(1, AppChangeKind.TaskCompleted));
            await Task.Delay(160);
            Assert.Equal(calls, query.SummaryCalls);
        }
        finally { SynchronizationContext.SetSynchronizationContext(original); }
    }

    private static async Task WaitForAsync(Func<bool> condition)
    { for (var i = 0; i < 20 && !condition(); i++) await Task.Delay(10); Assert.True(condition()); }

    private sealed class FakeQuery : IReviewQueryService
    {
        public List<ReviewRange> Ranges { get; } = []; public int SummaryCalls { get; private set; } public bool FailNext { get; set; }
        public Task<ReviewSummary> GetSummaryAsync(ReviewRange range, CancellationToken cancellationToken = default) { SummaryCalls++; Ranges.Add(range); if (FailNext) { FailNext = false; throw new InvalidOperationException(); } return Task.FromResult(new ReviewSummary(2, 2, 3660, 1800, true, 3, 1)); }
        public Task<IReadOnlyList<RecentCompletion>> GetRecentCompletedAsync(int limit = 20, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<RecentCompletion>>([new("event", DateTimeOffset.UtcNow, DateOnly.FromDateTime(DateTime.Today), "已删除任务快照", null, false)]);
        public Task<IReadOnlyList<DateBucketPoint>> GetCompletedTrendAsync(ReviewRange range, DayOfWeek weekStart, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<DateBucketPoint>>([]);
        public Task<IReadOnlyList<DateBucketPoint>> GetFocusTrendAsync(ReviewRange range, DayOfWeek weekStart, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<DateBucketPoint>>([]);
        public Task<IReadOnlyList<QuadrantValue>> GetCompletionByQuadrantAsync(ReviewRange range, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<QuadrantValue>>([]);
        public Task<IReadOnlyList<QuadrantValue>> GetFocusByQuadrantAsync(ReviewRange range, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<QuadrantValue>>([]);
    }
    private sealed class ImmediateContext : SynchronizationContext { public override void Post(SendOrPostCallback callback, object? state) => callback(state); }
}
