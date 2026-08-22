using Microsoft.Data.Sqlite;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteSettingsRepository : ISettingsRepository
{
    private readonly SqliteConnectionFactory factory;

    public SqliteSettingsRepository(SqliteConnectionFactory factory) => this.factory = factory ?? throw new ArgumentNullException(nameof(factory));

    public async Task<AppSettings> GetAsync(CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "SELECT key, value FROM settings;";
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        while (await reader.ReadAsync(cancellationToken)) values[reader.GetString(0)] = reader.GetString(1);
        var defaults = AppSettings.Default;
        return new AppSettings(
            ParseChoice(values, "theme", defaults.Theme, "System", "Light", "Dark"),
            ParseBool(values, "close_to_tray", defaults.CloseToTray),
            ParseBool(values, "launch_at_startup", defaults.LaunchAtStartup),
            ParseBool(values, "start_minimized", defaults.StartMinimized),
            ParseChoice(values, "global_hotkey", defaults.GlobalHotkey, "Ctrl+Alt+Q"),
            ParseNullableQuadrant(values, "quick_capture_destination", defaults.QuickCaptureQuadrantId),
            ParseEnum(values, "default_reminder", defaults.DefaultReminder),
            ParseInt(values, "focus_minutes", defaults.FocusMinutes, 1, 240),
            ParseInt(values, "short_break_minutes", defaults.ShortBreakMinutes, 1, 120),
            ParseInt(values, "long_break_minutes", defaults.LongBreakMinutes, 1, 120),
            ParseInt(values, "long_break_interval", defaults.LongBreakInterval, 2, 12),
            ParseBool(values, "auto_start_break", defaults.AutoStartBreak),
            ParseBool(values, "auto_start_focus", defaults.AutoStartFocus),
            ParseBool(values, "task_reminders_enabled", defaults.TaskRemindersEnabled),
            ParseBool(values, "focus_notifications_enabled", defaults.FocusNotificationsEnabled),
            ParseBool(values, "notification_sound_enabled", defaults.NotificationSoundEnabled),
            ParseEnum(values, "review_default_range", defaults.ReviewDefaultRange),
            ParseEnum(values, "week_start", defaults.WeekStart),
            ParseDouble(values, "sidebar_icon_size", defaults.SidebarIconSize, 16, 32));
    }

    public async Task SaveAsync(
        AppSettings settings,
        IReadOnlyList<QuadrantDefinition> quadrants,
        CancellationToken cancellationToken = default)
    {
        settings.Validate();
        await using var connection = await OpenAsync(cancellationToken);
        await using var transaction = connection.BeginTransaction();
        foreach (var pair in new Dictionary<string, string>
        {
            ["theme"] = settings.Theme,
            ["close_to_tray"] = settings.CloseToTray.ToString().ToLowerInvariant(),
            ["launch_at_startup"] = settings.LaunchAtStartup.ToString().ToLowerInvariant(),
            ["start_minimized"] = settings.StartMinimized.ToString().ToLowerInvariant(),
            ["global_hotkey"] = settings.GlobalHotkey,
            ["quick_capture_destination"] = settings.QuickCaptureQuadrantId is { } quadrantId ? $"Q{quadrantId}" : "Inbox",
            ["default_reminder"] = settings.DefaultReminder.ToString(),
            ["focus_minutes"] = settings.FocusMinutes.ToString(System.Globalization.CultureInfo.InvariantCulture),
            ["short_break_minutes"] = settings.ShortBreakMinutes.ToString(System.Globalization.CultureInfo.InvariantCulture),
            ["long_break_minutes"] = settings.LongBreakMinutes.ToString(System.Globalization.CultureInfo.InvariantCulture),
            ["long_break_interval"] = settings.LongBreakInterval.ToString(System.Globalization.CultureInfo.InvariantCulture),
            ["auto_start_break"] = settings.AutoStartBreak.ToString().ToLowerInvariant(),
            ["auto_start_focus"] = settings.AutoStartFocus.ToString().ToLowerInvariant(),
            ["task_reminders_enabled"] = settings.TaskRemindersEnabled.ToString().ToLowerInvariant(),
            ["focus_notifications_enabled"] = settings.FocusNotificationsEnabled.ToString().ToLowerInvariant(),
            ["notification_sound_enabled"] = settings.NotificationSoundEnabled.ToString().ToLowerInvariant(),
            ["review_default_range"] = settings.ReviewDefaultRange.ToString(),
            ["week_start"] = settings.WeekStart.ToString(),
            ["sidebar_icon_size"] = settings.SidebarIconSize.ToString(System.Globalization.CultureInfo.InvariantCulture)
        })
        {
            await using var command = connection.CreateCommand();
            command.Transaction = transaction;
            command.CommandText = "INSERT INTO settings (key, value) VALUES ($key, $value) ON CONFLICT(key) DO UPDATE SET value = excluded.value;";
            command.Parameters.AddWithValue("$key", pair.Key);
            command.Parameters.AddWithValue("$value", pair.Value);
            await command.ExecuteNonQueryAsync(cancellationToken);
        }

        foreach (var quadrant in quadrants)
        {
            await using var command = connection.CreateCommand();
            command.Transaction = transaction;
            command.CommandText = "UPDATE quadrants SET name = $name, subtitle = $subtitle WHERE id = $id;";
            command.Parameters.AddWithValue("$id", quadrant.Id);
            command.Parameters.AddWithValue("$name", quadrant.Name);
            command.Parameters.AddWithValue("$subtitle", quadrant.Subtitle);
            if (await command.ExecuteNonQueryAsync(cancellationToken) == 0)
            {
                throw new InvalidOperationException($"Quadrant {quadrant.Id} was not found.");
            }
        }

        await transaction.CommitAsync(cancellationToken);
    }

    private async Task<SqliteConnection> OpenAsync(CancellationToken cancellationToken)
    {
        var connection = factory.CreateConnection();
        await connection.OpenAsync(cancellationToken);
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, cancellationToken);
        return connection;
    }

    private static bool ParseBool(Dictionary<string, string> values, string key, bool fallback) =>
        values.TryGetValue(key, out var value) && bool.TryParse(value, out var parsed) ? parsed : fallback;

    private static int ParseInt(Dictionary<string, string> values, string key, int fallback, int minimum, int maximum) =>
        values.TryGetValue(key, out var value) && int.TryParse(value, System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out var parsed) && parsed >= minimum && parsed <= maximum ? parsed : fallback;

    private static double ParseDouble(Dictionary<string, string> values, string key, double fallback, double minimum, double maximum) =>
        values.TryGetValue(key, out var value) && double.TryParse(value, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var parsed) && parsed >= minimum && parsed <= maximum ? parsed : fallback;

    private static T ParseEnum<T>(Dictionary<string, string> values, string key, T fallback) where T : struct, Enum =>
        values.TryGetValue(key, out var value) && Enum.TryParse<T>(value, true, out var parsed) && Enum.IsDefined(parsed) ? parsed : fallback;

    private static string ParseChoice(Dictionary<string, string> values, string key, string fallback, params string[] choices) =>
        values.TryGetValue(key, out var value) && choices.Contains(value, StringComparer.OrdinalIgnoreCase) ? choices.First(choice => string.Equals(choice, value, StringComparison.OrdinalIgnoreCase)) : fallback;

    private static int? ParseNullableQuadrant(Dictionary<string, string> values, string key, int? fallback)
    {
        if (!values.TryGetValue(key, out var value)) return fallback;
        if (string.Equals(value, "Inbox", StringComparison.OrdinalIgnoreCase)) return null;
        return value.Length == 2 && (value[0] is 'Q' or 'q') && int.TryParse(value.AsSpan(1), out var quadrant) && quadrant is >= 1 and <= 4 ? quadrant : fallback;
    }
}
