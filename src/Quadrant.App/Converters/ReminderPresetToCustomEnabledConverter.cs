using System.Globalization;
using System.Windows.Data;
using Quadrant.Core.Enums;

namespace Quadrant.App.Converters;

public sealed class ReminderPresetToCustomEnabledConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture) => value is ReminderPreset.Custom;

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) => System.Windows.Data.Binding.DoNothing;
}
