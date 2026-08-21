using System.Globalization;
using System.Windows.Data;

namespace Quadrant.App.Converters;

public sealed class QuickCaptureDestinationToBooleanConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (parameter is not string destination) return false;
        return string.Equals(destination, "Inbox", StringComparison.Ordinal)
            ? value is null
            : int.TryParse(destination, CultureInfo.InvariantCulture, out var quadrantId) && value is int current && current == quadrantId;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is not true || parameter is not string destination) return System.Windows.Data.Binding.DoNothing;
        return string.Equals(destination, "Inbox", StringComparison.Ordinal)
            ? null!
            : int.TryParse(destination, CultureInfo.InvariantCulture, out var quadrantId)
                ? quadrantId
                : System.Windows.Data.Binding.DoNothing;
    }
}
