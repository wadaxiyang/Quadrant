using System.Globalization;

namespace Quadrant.Infrastructure.Notifications;

public sealed record NotificationActivation(string Action, long TaskId);

public static class NotificationActivationParser
{
    public static bool TryParse(string? argument, out NotificationActivation? activation)
    {
        activation = null;
        if (string.IsNullOrWhiteSpace(argument))
        {
            return false;
        }

        var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var part in argument.Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            var separator = part.IndexOf('=');
            if (separator <= 0)
            {
                continue;
            }

            var key = Uri.UnescapeDataString(part[..separator]);
            var value = Uri.UnescapeDataString(part[(separator + 1)..]);
            values[key] = value;
        }

        if (!values.TryGetValue("action", out var action) ||
            action is not ("complete" or "open" or "snooze10") ||
            !values.TryGetValue("taskId", out var taskIdText) ||
            !long.TryParse(taskIdText, NumberStyles.None, CultureInfo.InvariantCulture, out var taskId) ||
            taskId <= 0)
        {
            return false;
        }

        activation = new NotificationActivation(action, taskId);
        return true;
    }
}
