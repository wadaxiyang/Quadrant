using Quadrant.Core.Enums;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class ReviewInsightServiceTests
{
    [Fact]
    public void Creates_ordered_non_trivial_insights_and_suppresses_ties()
    {
        var current = new ReviewSummary(8, 2, 5400, 2700, true, 0, 0);
        var previous = new ReviewSummary(5, 1, 3600, 3600, true, 0, 0);
        var dashboard = new ReviewDashboard(
            ReviewRange.SevenDays,
            current,
            previous,
            [new(new DateOnly(2026, 8, 20), "2026-08-20", 2), new(new DateOnly(2026, 8, 21), "2026-08-21", 6)],
            [],
            [new(1, "Q1", 4), new(2, "Q2", 2), new(3, "Q3", 1), new(4, "Q4", 1)],
            [new(1, "Q1", 2700), new(2, "Q2", 2700), new(3, "Q3", 0), new(4, "Q4", 0)],
            new FocusReviewSummary(5400, 2, 2700, 3600, "Task", 5400, 2, 1, 2700),
            []);

        var insights = new ReviewInsightService().CreateInsights(dashboard);

        Assert.Equal(ReviewInsightKind.CompletionChange, insights[0].Kind);
        Assert.Equal(ReviewInsightKind.ImportantWorkShare, insights[1].Kind);
        Assert.Equal(ReviewInsightKind.HighestCompletionQuadrant, insights[2].Kind);
        Assert.DoesNotContain(insights, insight => insight.Kind == ReviewInsightKind.HighestFocusQuadrant);
        Assert.True(insights.Count <= 5);
    }

    [Fact]
    public void No_data_produces_no_insights()
    {
        var empty = new ReviewSummary(0, 0, 0, 0, false, 0, 0);
        var dashboard = new ReviewDashboard(ReviewRange.AllTime, empty, null, [], [], [], [],
            new FocusReviewSummary(0, 0, 0, 0, null, 0, 0, null, 0), []);

        Assert.Empty(new ReviewInsightService().CreateInsights(dashboard));
    }
}
