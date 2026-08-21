namespace Quadrant.Core.Services;

public enum LocalTimeResolutionError
{
    None,
    Invalid,
    Ambiguous
}

public static class LocalTimeResolver
{
    public static bool TryResolve(
        DateTime localDateTime,
        TimeZoneInfo timeZone,
        out DateTimeOffset value,
        out LocalTimeResolutionError error)
    {
        ArgumentNullException.ThrowIfNull(timeZone);
        var unspecified = DateTime.SpecifyKind(localDateTime, DateTimeKind.Unspecified);
        if (timeZone.IsInvalidTime(unspecified))
        {
            value = default;
            error = LocalTimeResolutionError.Invalid;
            return false;
        }

        if (timeZone.IsAmbiguousTime(unspecified))
        {
            value = default;
            error = LocalTimeResolutionError.Ambiguous;
            return false;
        }

        value = new DateTimeOffset(unspecified, timeZone.GetUtcOffset(unspecified));
        error = LocalTimeResolutionError.None;
        return true;
    }
}
