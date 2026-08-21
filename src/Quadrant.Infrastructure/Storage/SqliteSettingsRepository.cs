using Microsoft.Data.Sqlite;
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
        return new AppSettings(
            Get(values, "theme", "System"),
            bool.Parse(Get(values, "close_to_tray", "true")),
            bool.Parse(Get(values, "launch_at_startup", "false")),
            bool.Parse(Get(values, "start_minimized", "false")),
            Get(values, "global_hotkey", "Ctrl+Alt+Q"));
    }

    public async Task SaveAsync(AppSettings settings, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenAsync(cancellationToken);
        await using var transaction = connection.BeginTransaction();
        foreach (var pair in new Dictionary<string, string>
        {
            ["theme"] = settings.Theme,
            ["close_to_tray"] = settings.CloseToTray.ToString().ToLowerInvariant(),
            ["launch_at_startup"] = settings.LaunchAtStartup.ToString().ToLowerInvariant(),
            ["start_minimized"] = settings.StartMinimized.ToString().ToLowerInvariant(),
            ["global_hotkey"] = settings.GlobalHotkey
        })
        {
            await using var command = connection.CreateCommand();
            command.Transaction = transaction;
            command.CommandText = "INSERT INTO settings (key, value) VALUES ($key, $value) ON CONFLICT(key) DO UPDATE SET value = excluded.value;";
            command.Parameters.AddWithValue("$key", pair.Key);
            command.Parameters.AddWithValue("$value", pair.Value);
            await command.ExecuteNonQueryAsync(cancellationToken);
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

    private static string Get(Dictionary<string, string> values, string key, string fallback) => values.TryGetValue(key, out var value) ? value : fallback;
}
