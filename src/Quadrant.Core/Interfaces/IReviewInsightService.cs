using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface IReviewInsightService
{
    IReadOnlyList<ReviewInsight> CreateInsights(ReviewDashboard dashboard, int maximum = 5);
}
