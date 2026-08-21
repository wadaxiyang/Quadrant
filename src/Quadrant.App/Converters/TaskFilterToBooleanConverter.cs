using System.Globalization;
using System.Windows.Data;
using Quadrant.Core.Enums;

namespace Quadrant.App.Converters;

public sealed class TaskFilterToBooleanConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture) =>
        value is TaskFilter filter &&
        Enum.TryParse<TaskFilter>(System.Convert.ToString(parameter, CultureInfo.InvariantCulture), out var expected) &&
        filter == expected;

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) =>
        value is true && Enum.TryParse<TaskFilter>(System.Convert.ToString(parameter, CultureInfo.InvariantCulture), out var filter)
            ? filter
            : System.Windows.Data.Binding.DoNothing;
}
