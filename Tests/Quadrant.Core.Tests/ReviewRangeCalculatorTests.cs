using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class ReviewRangeCalculatorTests
{
    [Fact]
    public void Seven_days_includes_today_and_previous_six_local_dates()
    {
        var local = new DateTimeOffset(2026, 8, 21, 0, 15, 0, TimeSpan.FromHours(-7));
        var range = new ReviewRangeCalculator(new Clock(local)).GetRange(ReviewRange.SevenDays);

        Assert.Equal(new DateOnly(2026, 8, 15), range.LowerInclusive);
        Assert.Equal(new DateOnly(2026, 8, 22), range.UpperExclusive);
    }

    [Fact]
    public void Previous_range_has_same_length_and_ends_at_current_lower_bound()
    {
        var calculator = new ReviewRangeCalculator(new Clock(new DateTimeOffset(2026, 8, 21, 12, 0, 0, TimeSpan.Zero)));
        var current = calculator.GetRange(ReviewRange.ThirtyDays);
        var previous = calculator.GetPreviousRange(ReviewRange.ThirtyDays);

        Assert.NotNull(previous);
        Assert.Equal(current.LowerInclusive, previous!.UpperExclusive);
        Assert.Equal(30, previous.UpperExclusive.DayNumber - previous.LowerInclusive!.Value.DayNumber);
        Assert.Null(calculator.GetPreviousRange(ReviewRange.AllTime));
    }

    private sealed class Clock(DateTimeOffset local) : IClock
    {
        public DateTimeOffset UtcNow => local.ToUniversalTime(); public DateTimeOffset LocalNow => local;
        public DateOnly LocalDate => DateOnly.FromDateTime(local.Date); public TimeZoneInfo LocalTimeZone => TimeZoneInfo.Utc;
        public long GetTimestamp() => 0; public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }
}
