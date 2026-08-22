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
            Assert.Equal(7, viewModel.ActivityItems.Count);
            Assert.Equal([(int?)1, 2, 3, 4, null], viewModel.CompletedQuadrantRows.Select(point => point.QuadrantId));
            Assert.Equal("1 小时 01 分", viewModel.Kpis.Single(item => item.Label == "专注时间").Value);
            Assert.Equal("30 分", viewModel.Kpis.Single(item => item.Label == "平均专注").Value);
            Assert.Equal("3", viewModel.Kpis.Single(item => item.Label == "当前 Inbox").Value);
            Assert.Equal(4, viewModel.PrimaryKpis.Count);
            Assert.Equal(2, viewModel.CurrentStateKpis.Count);
            Assert.Equal(2, viewModel.Comparisons.Count);
            Assert.Equal("+2", viewModel.Comparisons.Single(item => item.MetricName == "完成任务").DeltaText);
            Assert.Equal(100, viewModel.ActivityItems.Max(item => item.RelativeValue));
            Assert.All(viewModel.CompletedQuadrantRows, item => Assert.InRange(item.ProgressValue, 0, 100));
            Assert.NotEmpty(viewModel.InsightItems);
            viewModel.SelectedRange = ReviewRange.ThirtyDays;
            await WaitForAsync(() => query.Ranges.Contains(ReviewRange.ThirtyDays));
            Assert.Equal(2, query.SummaryCalls);
            viewModel.SelectedRange = ReviewRange.AllTime;
            await WaitForAsync(() => query.SummaryCalls == 3);
            Assert.Equal(36, viewModel.ActivityItems.Count);
            Assert.All(viewModel.ActivityItems, item => Assert.Equal(0, item.RelativeValue));
            Assert.Empty(viewModel.Comparisons);
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
        public Task<ReviewDashboard> GetDashboardAsync(ReviewRange range, DayOfWeek weekStart, int recentLimit = 20, CancellationToken cancellationToken = default)
        {
            SummaryCalls++; Ranges.Add(range); if (FailNext) { FailNext = false; throw new InvalidOperationException(); }
            var current = new ReviewSummary(2, 2, 3660, 1800, true, 3, 1);
            var previous = range == ReviewRange.AllTime ? null : new ReviewSummary(0, 0, 0, 0, false, 0, 0);
            var pointCount = range == ReviewRange.AllTime ? 50 : 7;
            var completed = Enumerable.Range(0, pointCount).Select(index => new DateBucketPoint(new DateOnly(2026, 1, 1).AddDays(index), $"point-{index}", range == ReviewRange.AllTime ? 0 : index == 5 ? 1 : 0)).ToArray();
            var focus = Enumerable.Range(0, pointCount).Select(index => new DateBucketPoint(new DateOnly(2026, 1, 1).AddDays(index), $"point-{index}", range == ReviewRange.AllTime ? 0 : index == 6 ? 3660 : 0)).ToArray();
            var completionQuadrants = new QuadrantValue[] { new(1, "Q1", 2), new(2, "Q2", 0), new(3, "Q3", 0), new(4, "Q4", 0), new(null, "Inbox", 1) };
            var focusQuadrants = new QuadrantValue[] { new(1, "Q1", 3600), new(2, "Q2", 0), new(3, "Q3", 0), new(4, "Q4", 0), new(null, "Unlinked", 60) };
            var focusSummary = new FocusReviewSummary(3660, 2, 1830, 3600, "Task", 3600, 1, 1, 3600);
            RecentCompletion[] recent = [new("event", DateTimeOffset.UtcNow, DateOnly.FromDateTime(DateTime.Today), "已删除任务快照", null, false)];
            return Task.FromResult(new ReviewDashboard(range, current, previous, completed, focus, completionQuadrants, focusQuadrants, focusSummary, recent));
        }
        public Task<ReviewSummary> GetSummaryAsync(ReviewRange range, CancellationToken cancellationToken = default) { SummaryCalls++; Ranges.Add(range); if (FailNext) { FailNext = false; throw new InvalidOperationException(); } return Task.FromResult(new ReviewSummary(2, 2, 3660, 1800, true, 3, 1)); }
        public Task<IReadOnlyList<RecentCompletion>> GetRecentCompletedAsync(int limit = 20, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<RecentCompletion>>([new("event", DateTimeOffset.UtcNow, DateOnly.FromDateTime(DateTime.Today), "已删除任务快照", null, false)]);
        public Task<IReadOnlyList<DateBucketPoint>> GetCompletedTrendAsync(ReviewRange range, DayOfWeek weekStart, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<DateBucketPoint>>([new(new DateOnly(2026, 8, 20), "2026-08-20", 1), new(new DateOnly(2026, 8, 21), "2026-08-21", 2)]);
        public Task<IReadOnlyList<DateBucketPoint>> GetFocusTrendAsync(ReviewRange range, DayOfWeek weekStart, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<DateBucketPoint>>([new(new DateOnly(2026, 8, 20), "2026-08-20", 1800), new(new DateOnly(2026, 8, 21), "2026-08-21", 3600)]);
        public Task<IReadOnlyList<QuadrantValue>> GetCompletionByQuadrantAsync(ReviewRange range, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<QuadrantValue>>([new(1, "Q1", 2), new(null, "未分类", 1)]);
        public Task<IReadOnlyList<QuadrantValue>> GetFocusByQuadrantAsync(ReviewRange range, CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<QuadrantValue>>([new(1, "Q1", 3600), new(null, "未关联", 1800)]);
    }
    private sealed class ImmediateContext : SynchronizationContext { public override void Post(SendOrPostCallback callback, object? state) => callback(state); }
}
