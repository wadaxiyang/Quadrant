using System.Globalization;
using System.Text.Json;
using Microsoft.Data.Sqlite;
using Quadrant.Core.Interfaces;
using Quadrant.Infrastructure.Storage;
using Xunit;

namespace Quadrant.Infrastructure.Tests;

public sealed class SqliteDataMaintenanceServiceTests
{
    [Fact]
    public async Task Backup_is_integral_reopenable_and_atomically_replaces_existing_target()
    {
        await using var fixture = await Fixture.CreateAsync();
        await fixture.ExecuteAsync("INSERT INTO tasks(title,quadrant_id,note,is_completed,created_at,updated_at,recurrence_kind,recurrence_interval) VALUES ($title,1,$note,0,$now,$now,0,1);", ("$title", "中文"), ("$note", "quote \" ok"), ("$now", "2026-08-22T00:00:00.0000000+00:00"));
        var destination = Path.Combine(fixture.Directory, "backup.db");
        await File.WriteAllTextAsync(destination, "old target");

        await fixture.Service.BackupAsync(destination);

        await using var backup = new SqliteConnection(new SqliteConnectionStringBuilder { DataSource = destination, Mode = SqliteOpenMode.ReadOnly, Pooling = false }.ToString());
        await backup.OpenAsync();
        await using var command = backup.CreateCommand();
        command.CommandText = "SELECT title FROM tasks;";
        Assert.Equal("中文", await command.ExecuteScalarAsync());
        command.CommandText = "PRAGMA integrity_check;";
        Assert.Equal("ok", await command.ExecuteScalarAsync());
        Assert.Empty(Directory.GetFiles(fixture.Directory, ".*.tmp"));
    }

    [Fact]
    public async Task Json_export_has_versioned_ordered_utf8_envelope_and_preserves_nulls()
    {
        await using var fixture = await Fixture.CreateAsync();
        await fixture.ExecuteAsync("INSERT INTO tasks(title,quadrant_id,note,is_completed,created_at,updated_at,recurrence_kind,recurrence_interval) VALUES ($title,NULL,NULL,0,$now,$now,0,1);", ("$title", "中文 \"task\""), ("$now", "2026-08-22T00:00:00.0000000+00:00"));
        var destination = Path.Combine(fixture.Directory, "export.json");

        await fixture.Service.ExportJsonAsync(destination);

        using var document = JsonDocument.Parse(await File.ReadAllBytesAsync(destination));
        var root = document.RootElement;
        Assert.Equal(1, root.GetProperty("formatVersion").GetInt32());
        Assert.Equal("中文 \"task\"", root.GetProperty("tasks")[0].GetProperty("title").GetString());
        Assert.Equal(JsonValueKind.Null, root.GetProperty("tasks")[0].GetProperty("quadrant_id").ValueKind);
        Assert.True(root.TryGetProperty("portableSettings", out _));
        Assert.True(root.TryGetProperty("focusSessions", out _));
        Assert.True(root.TryGetProperty("completionEvents", out _));
        Assert.Empty(Directory.GetFiles(fixture.Directory, ".*.tmp"));
    }

    [Fact]
    public async Task Cleanup_scopes_are_distinct_and_reset_restores_defaults()
    {
        await using var fixture = await Fixture.CreateAsync();
        const string now = "2026-08-22T00:00:00.0000000+00:00";
        await fixture.ExecuteAsync("INSERT INTO tasks(title,quadrant_id,is_completed,completed_at,created_at,updated_at,recurrence_kind,recurrence_interval) VALUES ('done',1,1,$now,$now,$now,0,1);", ("$now", now));
        await fixture.ExecuteAsync("INSERT INTO task_completion_events(id,task_id,completed_at_utc,completed_local_date,task_title_snapshot,was_overdue) VALUES ('e',1,$now,'2026-08-22','done',0);", ("$now", now));
        await fixture.ExecuteAsync("INSERT INTO focus_sessions(id,mode,started_at_utc,duration_seconds,status,pomodoro_kind,created_local_date) VALUES ('f',2,$now,60,3,1,'2026-08-22');", ("$now", now));

        await fixture.Service.ClearFocusHistoryAsync();
        Assert.Equal(0, await fixture.CountAsync("focus_sessions"));
        Assert.Equal(1, await fixture.CountAsync("task_completion_events"));
        await fixture.Service.ClearCompletionHistoryAsync();
        Assert.Equal(1, await fixture.CountAsync("tasks"));
        await fixture.ExecuteAsync("UPDATE quadrants SET name='changed' WHERE id=1; INSERT INTO settings(key,value) VALUES('focus_minutes','99');");
        await fixture.Service.ResetAllAsync();

        Assert.Equal(0, await fixture.CountAsync("tasks"));
        Assert.Equal("重要且紧急", await fixture.ScalarAsync("SELECT name FROM quadrants WHERE id=1;"));
        Assert.Equal("System", await fixture.ScalarAsync("SELECT value FROM settings WHERE key='theme';"));
        Assert.Null(await fixture.ScalarAsync("SELECT value FROM settings WHERE key='focus_minutes';"));
    }

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset UtcNow => new(2026, 8, 22, 1, 2, 3, TimeSpan.Zero);
        public DateTimeOffset LocalNow => UtcNow;
        public DateOnly LocalDate => new(2026, 8, 22);
        public TimeZoneInfo LocalTimeZone => TimeZoneInfo.Utc;
        public long GetTimestamp() => 0;
        public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }

    private sealed class Fixture : IAsyncDisposable
    {
        private Fixture(string directory, SqliteConnectionFactory factory) { Directory = directory; Factory = factory; Service = new(factory, new FixedClock()); }
        public string Directory { get; }
        public SqliteConnectionFactory Factory { get; }
        public SqliteDataMaintenanceService Service { get; }
        public static async Task<Fixture> CreateAsync()
        {
            var directory = Path.Combine(Path.GetTempPath(), "QuadrantMaintenanceTests", Guid.NewGuid().ToString("N"));
            System.IO.Directory.CreateDirectory(directory);
            var factory = new SqliteConnectionFactory(Path.Combine(directory, "quadrant.db"), pooling: false);
            await new SqliteDatabaseInitializer(factory).InitializeAsync();
            return new(directory, factory);
        }
        public async Task ExecuteAsync(string sql, params (string Name, object Value)[] parameters)
        {
            await using var connection = Factory.CreateConnection(); await connection.OpenAsync();
            await using var command = connection.CreateCommand(); command.CommandText = sql;
            foreach (var parameter in parameters) command.Parameters.AddWithValue(parameter.Name, parameter.Value);
            await command.ExecuteNonQueryAsync();
        }
        public async Task<int> CountAsync(string table) => Convert.ToInt32(await ScalarAsync($"SELECT COUNT(*) FROM {table};"), CultureInfo.InvariantCulture);
        public async Task<object?> ScalarAsync(string sql)
        {
            await using var connection = Factory.CreateConnection(); await connection.OpenAsync();
            await using var command = connection.CreateCommand(); command.CommandText = sql; return await command.ExecuteScalarAsync();
        }
        public ValueTask DisposeAsync() { System.IO.Directory.Delete(Directory, recursive: true); return ValueTask.CompletedTask; }
    }
}
