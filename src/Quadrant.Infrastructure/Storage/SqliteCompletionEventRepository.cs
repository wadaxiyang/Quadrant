using Microsoft.Data.Sqlite;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteCompletionEventRepository(SqliteConnectionFactory connectionFactory) : ICompletionEventRepository
{
    public async Task CreateAsync(CompletionEvent value, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """INSERT INTO task_completion_events (id,task_id,completed_at_utc,completed_local_date,quadrant_snapshot,task_title_snapshot,due_at_utc_snapshot,planned_date_snapshot,estimated_minutes_snapshot,was_overdue,reverted_at_utc) VALUES ($id,$task,$completed,$date,$quadrant,$title,$due,$planned,$estimate,$overdue,$reverted);""";
        command.Parameters.AddWithValue("$id", value.Id); command.Parameters.AddWithValue("$task", SqliteValueConverter.ToDbValue(value.TaskId));
        command.Parameters.AddWithValue("$completed", SqliteValueConverter.FormatUtc(value.CompletedAtUtc)); command.Parameters.AddWithValue("$date", SqliteValueConverter.FormatDateOnly(value.CompletedLocalDate));
        command.Parameters.AddWithValue("$quadrant", SqliteValueConverter.ToDbValue(value.QuadrantSnapshot)); command.Parameters.AddWithValue("$title", value.TaskTitleSnapshot);
        command.Parameters.AddWithValue("$due", SqliteValueConverter.ToDbValue(value.DueAtUtcSnapshot)); command.Parameters.AddWithValue("$planned", SqliteValueConverter.ToDbValue(value.PlannedDateSnapshot));
        command.Parameters.AddWithValue("$estimate", SqliteValueConverter.ToDbValue(value.EstimatedMinutesSnapshot)); command.Parameters.AddWithValue("$overdue", value.WasOverdue ? 1 : 0); command.Parameters.AddWithValue("$reverted", SqliteValueConverter.ToDbValue(value.RevertedAtUtc));
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    public async Task<CompletionEvent?> GetByIdAsync(string id, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenAsync(cancellationToken); await using var command = connection.CreateCommand(); command.CommandText = "SELECT * FROM task_completion_events WHERE id=$id;"; command.Parameters.AddWithValue("$id", id);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken); if (!await reader.ReadAsync(cancellationToken)) return null;
        return new CompletionEvent(reader.GetString(reader.GetOrdinal("id")), ReadLong(reader,"task_id"), SqliteValueConverter.ParseDateTimeOffset(reader["completed_at_utc"]), SqliteValueConverter.ParseDateOnly(reader["completed_local_date"]), ReadInt(reader,"quadrant_snapshot"), reader.GetString(reader.GetOrdinal("task_title_snapshot")), ReadTime(reader,"due_at_utc_snapshot"), ReadDate(reader,"planned_date_snapshot"), ReadInt(reader,"estimated_minutes_snapshot"), reader.GetInt32(reader.GetOrdinal("was_overdue")) != 0, ReadTime(reader,"reverted_at_utc"));
    }

    private async Task<SqliteConnection> OpenAsync(CancellationToken ct) { var c=connectionFactory.CreateConnection(); await c.OpenAsync(ct); await SqliteDatabaseInitializer.ConfigureConnectionAsync(c,ct); return c; }
    private static long? ReadLong(SqliteDataReader r,string n)=>r[n] is DBNull?null:Convert.ToInt64(r[n]);
    private static int? ReadInt(SqliteDataReader r,string n)=>r[n] is DBNull?null:Convert.ToInt32(r[n]);
    private static DateOnly? ReadDate(SqliteDataReader r,string n)=>r[n] is DBNull?null:SqliteValueConverter.ParseDateOnly(r[n]);
    private static DateTimeOffset? ReadTime(SqliteDataReader r,string n)=>r[n] is DBNull?null:SqliteValueConverter.ParseDateTimeOffset(r[n]);
}
