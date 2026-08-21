using System.Globalization;
using System.Windows.Data;
using Quadrant.Core.Enums;

namespace Quadrant.App.Converters;

public sealed class ReminderPresetDisplayConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture) => value switch
    {
        ReminderPreset.None => "不提醒",
        ReminderPreset.AtDueTime => "到期时",
        ReminderPreset.TenMinutesBefore => "提前 10 分钟",
        ReminderPreset.OneHourBefore => "提前 1 小时",
        ReminderPreset.OneDayBefore => "提前 1 天",
        ReminderPreset.Custom => "自定义",
        _ => value?.ToString() ?? string.Empty
    };

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) => System.Windows.Data.Binding.DoNothing;
}
