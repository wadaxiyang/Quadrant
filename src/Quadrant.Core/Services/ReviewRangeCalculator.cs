using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class ReviewRangeCalculator(IClock clock)
{
    public ReviewDateRange GetRange(ReviewRange range)
    {
        var tomorrow = clock.LocalDate.AddDays(1);
        return range switch
        {
            ReviewRange.SevenDays => new ReviewDateRange(tomorrow.AddDays(-7), tomorrow),
            ReviewRange.ThirtyDays => new ReviewDateRange(tomorrow.AddDays(-30), tomorrow),
            ReviewRange.NinetyDays => new ReviewDateRange(tomorrow.AddDays(-90), tomorrow),
            ReviewRange.AllTime => new ReviewDateRange(null, tomorrow),
            _ => throw new ArgumentOutOfRangeException(nameof(range))
        };
    }
}
