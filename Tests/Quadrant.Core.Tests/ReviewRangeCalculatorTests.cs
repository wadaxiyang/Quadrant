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

    private sealed class Clock(DateTimeOffset local) : IClock
    {
        public DateTimeOffset UtcNow => local.ToUniversalTime(); public DateTimeOffset LocalNow => local;
        public DateOnly LocalDate => DateOnly.FromDateTime(local.Date); public TimeZoneInfo LocalTimeZone => TimeZoneInfo.Utc;
        public long GetTimestamp() => 0; public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }
}
