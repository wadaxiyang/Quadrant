namespace Quadrant.Core.Models;

public sealed record FocusDaySummary(long TotalSeconds, int SessionCount)
{
    public static FocusDaySummary Empty { get; } = new(0, 0);
}
