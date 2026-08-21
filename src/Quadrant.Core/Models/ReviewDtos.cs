namespace Quadrant.Core.Models;

public sealed record ReviewSummary(
    int CompletedTaskCount,
    int ProductiveFocusSessionCount,
    long TotalFocusSeconds,
    int AverageFocusSeconds,
    bool HasFocusData,
    int CurrentInboxCount,
    int CurrentOverdueCount);

public sealed record DateBucketPoint(DateOnly StartDate, string LabelKey, long Value);

public sealed record QuadrantValue(int? QuadrantId, string LabelKey, long Value);

public sealed record RecentCompletion(
    string EventId,
    DateTimeOffset CompletedAtUtc,
    DateOnly CompletedLocalDate,
    string TaskTitleSnapshot,
    int? QuadrantSnapshot,
    bool WasOverdue);

public sealed record ReviewDateRange(DateOnly? LowerInclusive, DateOnly UpperExclusive)
{
    public bool IsAllTime => LowerInclusive is null;
}
