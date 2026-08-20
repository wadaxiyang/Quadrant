using System.Globalization;

namespace Quadrant.Infrastructure.Storage;

internal static class SqliteValueConverter
{
    public static string Format(DateTimeOffset value) =>
        value.ToString("O", CultureInfo.InvariantCulture);

    public static DateTimeOffset ParseDateTimeOffset(object value) =>
        DateTimeOffset.Parse(Convert.ToString(value, CultureInfo.InvariantCulture)!, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind);

    public static object ToDbValue(DateTimeOffset? value) =>
        value is null ? DBNull.Value : Format(value.Value);

    public static object ToDbValue(string? value) =>
        value is null ? DBNull.Value : value;
}
