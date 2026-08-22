using System.Globalization;
using System.Text.Json;
using Microsoft.Data.Sqlite;
using Quadrant.Core.Interfaces;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteDataMaintenanceService(SqliteConnectionFactory connectionFactory, IClock clock) : IDataMaintenanceService
{
    private static readonly JsonWriterOptions WriterOptions = new() { Indented = true };

    public async Task BackupAsync(string destinationPath, CancellationToken cancellationToken = default)
    {
        var destination = ValidateDestination(destinationPath, ".db");
        var temporary = CreateTemporaryPath(destination);
        try
        {
            await Task.Run(() =>
            {
                cancellationToken.ThrowIfCancellationRequested();
                using var source = connectionFactory.CreateConnection();
                using var target = CreateConnection(temporary, SqliteOpenMode.ReadWriteCreate);
                source.Open();
                target.Open();
                source.BackupDatabase(target);
            }, CancellationToken.None);

            cancellationToken.ThrowIfCancellationRequested();
            await ValidateBackupAsync(temporary, cancellationToken);
            CommitTemporary(temporary, destination);
        }
        finally
        {
            TryDelete(temporary);
        }
    }

    public async Task ExportJsonAsync(string destinationPath, CancellationToken cancellationToken = default)
    {
        var destination = ValidateDestination(destinationPath, ".json");
        var temporary = CreateTemporaryPath(destination);
        try
        {
            await using (var stream = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None, 64 * 1024, FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                await using var connection = connectionFactory.CreateConnection();
                await connection.OpenAsync(cancellationToken);
                await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, cancellationToken);
                using (var writer = new Utf8JsonWriter(stream, WriterOptions))
                {
                    writer.WriteStartObject();
                    writer.WriteNumber("formatVersion", 1);
                    writer.WriteString("exportedAtUtc", clock.UtcNow.ToUniversalTime());
                    await WriteRowsAsync(writer, connection, "tasks", "SELECT * FROM tasks ORDER BY id;", cancellationToken);
                    await WriteRowsAsync(writer, connection, "quadrants", "SELECT * FROM quadrants ORDER BY id;", cancellationToken);
                    await WriteRowsAsync(writer, connection, "portableSettings", "SELECT key, value FROM settings ORDER BY key;", cancellationToken);
                    await WriteRowsAsync(writer, connection, "focusSessions", "SELECT * FROM focus_sessions ORDER BY started_at_utc, id;", cancellationToken);
                    await WriteRowsAsync(writer, connection, "completionEvents", "SELECT * FROM task_completion_events ORDER BY completed_at_utc, id;", cancellationToken);
                    writer.WriteEndObject();
                    await writer.FlushAsync(cancellationToken);
                }
                await stream.FlushAsync(cancellationToken);
                stream.Flush(flushToDisk: true);
            }
            CommitTemporary(temporary, destination);
        }
        finally
        {
            TryDelete(temporary);
        }
    }

    public Task ClearFocusHistoryAsync(CancellationToken cancellationToken = default) =>
        ExecuteTransactionAsync("DELETE FROM focus_sessions;", cancellationToken);

    public Task ClearCompletionHistoryAsync(CancellationToken cancellationToken = default) =>
        ExecuteTransactionAsync("DELETE FROM task_completion_events;", cancellationToken);

    public Task ResetAllAsync(CancellationToken cancellationToken = default) => ExecuteTransactionAsync(
        """
        DELETE FROM focus_sessions;
        DELETE FROM task_completion_events;
        DELETE FROM tasks;
        DELETE FROM settings;
        INSERT INTO settings (key,value) VALUES
          ('theme','System'),('close_to_tray','true'),('launch_at_startup','false'),('start_minimized','false'),('global_hotkey','Ctrl+Alt+Q');
        UPDATE quadrants SET name='重要且紧急', subtitle='立即处理' WHERE id=1;
        UPDATE quadrants SET name='重要不紧急', subtitle='规划推进' WHERE id=2;
        UPDATE quadrants SET name='紧急不重要', subtitle='简化或委派' WHERE id=3;
        UPDATE quadrants SET name='不重要不紧急', subtitle='删除或延后' WHERE id=4;
        """, cancellationToken, requireNoActiveFocus: true);

    private async Task ExecuteTransactionAsync(string sql, CancellationToken cancellationToken, bool requireNoActiveFocus = false)
    {
        await using var connection = connectionFactory.CreateConnection();
        await connection.OpenAsync(cancellationToken);
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, cancellationToken);
        await using var transaction = connection.BeginTransaction();
        if (requireNoActiveFocus)
        {
            await using var check = connection.CreateCommand();
            check.Transaction = transaction;
            check.CommandText = "SELECT EXISTS(SELECT 1 FROM focus_sessions WHERE status IN (1,2));";
            if (Convert.ToInt32(await check.ExecuteScalarAsync(cancellationToken), CultureInfo.InvariantCulture) != 0)
                throw new InvalidOperationException("请先结束或取消当前 Focus，再重置全部数据。");
        }
        await using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = sql;
        await command.ExecuteNonQueryAsync(cancellationToken);
        await transaction.CommitAsync(cancellationToken);
    }

    private async Task ValidateBackupAsync(string path, CancellationToken cancellationToken)
    {
        await using var connection = CreateConnection(path, SqliteOpenMode.ReadOnly);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1;";
        var version = Convert.ToInt32(await command.ExecuteScalarAsync(cancellationToken), CultureInfo.InvariantCulture);
        if (version != SqliteDatabaseInitializer.CurrentSchemaVersion)
            throw new InvalidDataException($"Backup schema version {version} is not supported.");
        command.CommandText = "PRAGMA integrity_check;";
        var result = Convert.ToString(await command.ExecuteScalarAsync(cancellationToken), CultureInfo.InvariantCulture);
        if (!string.Equals(result, "ok", StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException($"Backup integrity check failed: {result}");
    }

    private string ValidateDestination(string path, string extension)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        var full = Path.GetFullPath(path);
        if (string.Equals(full, Path.GetFullPath(connectionFactory.DatabasePath), StringComparison.OrdinalIgnoreCase))
            throw new ArgumentException("目标不能是当前正在使用的数据库。", nameof(path));
        if (!string.Equals(Path.GetExtension(full), extension, StringComparison.OrdinalIgnoreCase))
            throw new ArgumentException($"目标文件必须使用 {extension} 扩展名。", nameof(path));
        var directory = Path.GetDirectoryName(full) ?? throw new ArgumentException("目标目录无效。", nameof(path));
        if (!Directory.Exists(directory)) throw new DirectoryNotFoundException(directory);
        return full;
    }

    private static string CreateTemporaryPath(string destination) =>
        Path.Combine(Path.GetDirectoryName(destination)!, $".{Path.GetFileName(destination)}.{Guid.NewGuid():N}.tmp");

    private static SqliteConnection CreateConnection(string path, SqliteOpenMode mode) => new(new SqliteConnectionStringBuilder
    {
        DataSource = path, Mode = mode, Cache = SqliteCacheMode.Default, Pooling = false
    }.ToString());

    private static void CommitTemporary(string temporary, string destination)
    {
        if (File.Exists(destination)) File.Replace(temporary, destination, null);
        else File.Move(temporary, destination);
    }

    private static void TryDelete(string path)
    {
        try { if (File.Exists(path)) File.Delete(path); }
        catch { /* Best-effort cleanup must not replace the original failure. */ }
    }

    private static async Task WriteRowsAsync(Utf8JsonWriter writer, SqliteConnection connection, string propertyName, string sql, CancellationToken cancellationToken)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        await using var command = connection.CreateCommand();
        command.CommandText = sql;
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        while (await reader.ReadAsync(cancellationToken))
        {
            cancellationToken.ThrowIfCancellationRequested();
            writer.WriteStartObject();
            for (var i = 0; i < reader.FieldCount; i++)
            {
                writer.WritePropertyName(reader.GetName(i));
                if (reader.IsDBNull(i)) writer.WriteNullValue();
                else WriteValue(writer, reader.GetValue(i));
            }
            writer.WriteEndObject();
        }
        writer.WriteEndArray();
    }

    private static void WriteValue(Utf8JsonWriter writer, object value)
    {
        switch (value)
        {
            case long number: writer.WriteNumberValue(number); break;
            case int number: writer.WriteNumberValue(number); break;
            case double number: writer.WriteNumberValue(number); break;
            case byte[] bytes: writer.WriteBase64StringValue(bytes); break;
            default: writer.WriteStringValue(Convert.ToString(value, CultureInfo.InvariantCulture)); break;
        }
    }
}
