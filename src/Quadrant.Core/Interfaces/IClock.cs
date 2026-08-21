namespace Quadrant.Core.Interfaces;

public interface IClock
{
    DateTimeOffset UtcNow { get; }

    DateTimeOffset LocalNow { get; }

    DateOnly LocalDate { get; }

    TimeZoneInfo LocalTimeZone { get; }

    long GetTimestamp();

    TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp);
}
