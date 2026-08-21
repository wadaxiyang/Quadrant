using Quadrant.Core.Interfaces;

namespace Quadrant.Infrastructure.Windows;

public sealed class SystemClock : IClock
{
    private readonly TimeProvider timeProvider;

    public SystemClock(TimeProvider? timeProvider = null)
    {
        this.timeProvider = timeProvider ?? TimeProvider.System;
    }

    public DateTimeOffset UtcNow => timeProvider.GetUtcNow();

    public DateTimeOffset LocalNow => timeProvider.GetLocalNow();

    public DateOnly LocalDate => DateOnly.FromDateTime(LocalNow.Date);

    public TimeZoneInfo LocalTimeZone => timeProvider.LocalTimeZone;

    public long GetTimestamp() => timeProvider.GetTimestamp();

    public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) =>
        timeProvider.GetElapsedTime(startingTimestamp, endingTimestamp);
}
