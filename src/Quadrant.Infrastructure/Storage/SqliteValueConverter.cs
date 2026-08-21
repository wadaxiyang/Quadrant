using System.Globalization;

namespace Quadrant.Infrastructure.Storage;

internal static class SqliteValueConverter
{
    public static string Format(DateTimeOffset value) =>
        value.ToString("O", CultureInfo.InvariantCulture);

    public static string FormatUtc(DateTimeOffset value) =>
        value.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture);

    public static string FormatDateOnly(DateOnly value) =>
        value.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);

    public static DateOnly ParseDateOnly(object value) =>
        DateOnly.ParseExact(Convert.ToString(value, CultureInfo.InvariantCulture)!, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None);

    public static DateTimeOffset ParseDateTimeOffset(object value) =>
        DateTimeOffset.Parse(Convert.ToString(value, CultureInfo.InvariantCulture)!, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind);

    public static object ToDbValue(DateTimeOffset? value) =>
        value is null ? DBNull.Value : FormatUtc(value.Value);

    public static object ToDbValue(string? value) =>
        value is null ? DBNull.Value : value;

    public static object ToDbValue(DateOnly? value) =>
        value is null ? DBNull.Value : FormatDateOnly(value.Value);

    public static object ToDbValue(int? value) => (object?)value ?? DBNull.Value;

    public static object ToDbValue(long? value) => (object?)value ?? DBNull.Value;
}
