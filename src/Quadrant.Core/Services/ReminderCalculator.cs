using Quadrant.Core.Enums;

namespace Quadrant.Core.Services;

public static class ReminderCalculator
{
    public static DateTimeOffset? Calculate(
        ReminderPreset preset,
        DateTimeOffset? due,
        DateTimeOffset? custom)
    {
        return preset switch
        {
            ReminderPreset.None => null,
            ReminderPreset.AtDueTime => due,
            ReminderPreset.TenMinutesBefore => due?.AddMinutes(-10),
            ReminderPreset.OneHourBefore => due?.AddHours(-1),
            ReminderPreset.OneDayBefore => due?.AddDays(-1),
            ReminderPreset.Custom => custom,
            _ => throw new ArgumentOutOfRangeException(nameof(preset), preset, "Unknown reminder preset.")
        };
    }
}
