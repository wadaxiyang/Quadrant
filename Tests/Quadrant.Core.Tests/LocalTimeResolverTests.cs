using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class LocalTimeResolverTests
{
    private static readonly TimeZoneInfo TestZone = CreateTestZone();

    [Fact]
    public void Resolves_an_ordinary_local_time()
    {
        var local = new DateTime(2026, 2, 1, 9, 30, 0, DateTimeKind.Unspecified);

        var result = LocalTimeResolver.TryResolve(local, TestZone, out var resolved, out var error);

        Assert.True(result);
        Assert.Equal(LocalTimeResolutionError.None, error);
        Assert.Equal(local, resolved.DateTime);
    }

    [Fact]
    public void Rejects_invalid_and_ambiguous_daylight_saving_times()
    {
        var invalid = new DateTime(2026, 3, 8, 2, 30, 0, DateTimeKind.Unspecified);
        var ambiguous = new DateTime(2026, 11, 1, 1, 30, 0, DateTimeKind.Unspecified);

        Assert.False(LocalTimeResolver.TryResolve(invalid, TestZone, out _, out var invalidError));
        Assert.Equal(LocalTimeResolutionError.Invalid, invalidError);
        Assert.False(LocalTimeResolver.TryResolve(ambiguous, TestZone, out _, out var ambiguousError));
        Assert.Equal(LocalTimeResolutionError.Ambiguous, ambiguousError);
    }

    private static TimeZoneInfo CreateTestZone()
    {
        var daylightDelta = TimeSpan.FromHours(1);
        var start = TimeZoneInfo.TransitionTime.CreateFloatingDateRule(
            new DateTime(1, 1, 1, 2, 0, 0), 3, 2, DayOfWeek.Sunday);
        var end = TimeZoneInfo.TransitionTime.CreateFloatingDateRule(
            new DateTime(1, 1, 1, 2, 0, 0), 11, 1, DayOfWeek.Sunday);
        var rule = TimeZoneInfo.AdjustmentRule.CreateAdjustmentRule(
            new DateTime(2020, 1, 1),
            new DateTime(2030, 12, 31),
            daylightDelta,
            start,
            end);
        return TimeZoneInfo.CreateCustomTimeZone(
            "Quadrant-Test-DST",
            TimeSpan.FromHours(-8),
            "Quadrant Test DST",
            "Quadrant Test Standard",
            "Quadrant Test Daylight",
            [rule]);
    }
}
