namespace Quadrant.Core.Models;

public sealed record ReviewSummary(
    int CompletedTaskCount,
    int ProductiveFocusSessionCount,
    long TotalFocusSeconds,
    int AverageFocusSeconds,
    bool HasFocusData,
    int CurrentInboxCount,
    int CurrentOverdueCount);

public sealed record ReviewDashboard(
    ReviewRange Range,
    ReviewSummary Current,
    ReviewSummary? Previous,
    IReadOnlyList<DateBucketPoint> CompletedActivity,
    IReadOnlyList<DateBucketPoint> FocusActivity,
    IReadOnlyList<QuadrantValue> CompletedByQuadrant,
    IReadOnlyList<QuadrantValue> FocusByQuadrant,
    FocusReviewSummary FocusSummary,
    IReadOnlyList<RecentCompletion> RecentCompleted);

public sealed record FocusReviewSummary(
    long TotalFocusSeconds,
    int SessionCount,
    long AverageSessionSeconds,
    long LongestSessionSeconds,
    string? MostFocusedTaskTitle,
    long MostFocusedTaskSeconds,
    int MostFocusedTaskSessions,
    int? MostFocusedQuadrantId,
    long MostFocusedQuadrantSeconds);

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

public enum ReviewInsightKind
{
    CompletionChange,
    ImportantWorkShare,
    HighestCompletionQuadrant,
    HighestFocusQuadrant,
    MostActiveDay,
    FocusChange,
    AverageSession
}

public enum ReviewInsightTone { Neutral, Positive, Attention }

public sealed record ReviewInsight(ReviewInsightKind Kind, string Text, ReviewInsightTone Tone);
