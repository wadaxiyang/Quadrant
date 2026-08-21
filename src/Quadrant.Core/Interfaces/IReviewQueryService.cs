using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface IReviewQueryService
{
    Task<ReviewSummary> GetSummaryAsync(ReviewRange range, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<DateBucketPoint>> GetCompletedTrendAsync(ReviewRange range, DayOfWeek weekStart, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<DateBucketPoint>> GetFocusTrendAsync(ReviewRange range, DayOfWeek weekStart, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<QuadrantValue>> GetCompletionByQuadrantAsync(ReviewRange range, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<QuadrantValue>> GetFocusByQuadrantAsync(ReviewRange range, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<RecentCompletion>> GetRecentCompletedAsync(int limit = 20, CancellationToken cancellationToken = default);
}
