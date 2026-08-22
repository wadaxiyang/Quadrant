using Microsoft.Data.Sqlite;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteFocusSessionRepository(SqliteConnectionFactory connectionFactory) : IFocusSessionRepository
{
    public async Task<FocusSession?> GetCurrentAsync(CancellationToken cancellationToken = default)
    {
        await using var c = await OpenAsync(cancellationToken); return await ReadOneAsync(c, null, "SELECT * FROM focus_sessions WHERE status IN ($running,$paused) ORDER BY started_at_utc DESC LIMIT 1;", cancellationToken, ("$running", (object)(int)FocusStatus.Running), ("$paused", (object)(int)FocusStatus.Paused));
    }

    public async Task<FocusSession?> CreateIfNoCurrentAsync(FocusSession value, CancellationToken cancellationToken = default)
    {
        await using var c = await OpenAsync(cancellationToken); await using var tx = c.BeginTransaction(deferred: false);
        var current = await ReadOneAsync(c, tx, "SELECT * FROM focus_sessions WHERE status IN ($running,$paused) LIMIT 1;", cancellationToken, ("$running", (object)(int)FocusStatus.Running), ("$paused", (object)(int)FocusStatus.Paused));
        if (current is not null) return null;
        if (value.TaskId is { } taskId)
        {
            var snapshot = await ReadTaskSnapshotAsync(c, tx, taskId, cancellationToken)
                ?? throw new Quadrant.Core.Services.TaskValidationException("Focus task was not found.");
            if (snapshot.IsCompleted || snapshot.QuadrantId is null)
            {
                throw new Quadrant.Core.Services.TaskValidationException("Focus task must be active and classified.");
            }
            value = value with { TaskTitleSnapshot = snapshot.Title, QuadrantSnapshot = snapshot.QuadrantId };
        }
        await InsertAsync(c, tx, value, cancellationToken); await tx.CommitAsync(cancellationToken); return value;
    }

    public async Task<FocusSession?> GetByIdAsync(string id, CancellationToken cancellationToken = default)
    {
        await using var c=await OpenAsync(cancellationToken); return await ReadOneAsync(c, null, "SELECT * FROM focus_sessions WHERE id=$id;", cancellationToken, ("$id", id));
    }
    public async Task<FocusSession?> TransitionAsync(FocusSession value, FocusStatus expectedStatus, CancellationToken cancellationToken = default)
    { await using var c=await OpenAsync(cancellationToken); await using var x=c.CreateCommand(); x.CommandText="""UPDATE focus_sessions SET active_segment_started_utc=$active,ended_at_utc=$ended,target_end_at_utc=$target,duration_seconds=$duration,status=$status WHERE id=$id AND status=$expected RETURNING *;"""; AddParameters(x,value); x.Parameters.AddWithValue("$expected",(int)expectedStatus); await using var r=await x.ExecuteReaderAsync(cancellationToken); return await r.ReadAsync(cancellationToken)?Map(r):null; }
    public async Task<IReadOnlyList<FocusSession>> GetRecentAsync(int limit = 5, CancellationToken cancellationToken = default)
    { if(limit is < 1 or > 50) throw new ArgumentOutOfRangeException(nameof(limit)); await using var c=await OpenAsync(cancellationToken); await using var x=c.CreateCommand(); x.CommandText="SELECT * FROM focus_sessions ORDER BY started_at_utc DESC LIMIT $limit;";x.Parameters.AddWithValue("$limit",limit);await using var r=await x.ExecuteReaderAsync(cancellationToken);var values=new List<FocusSession>();while(await r.ReadAsync(cancellationToken))values.Add(Map(r));return values; }
    public async Task<FocusDaySummary> GetProductiveSummaryAsync(DateOnly localDate, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            SELECT COUNT(*), COALESCE(SUM(duration_seconds), 0)
            FROM focus_sessions
            WHERE created_local_date = $date
              AND status = $completed
              AND (mode = $stopwatch OR (mode = $pomodoro AND pomodoro_kind = $focus));
            """;
        command.Parameters.AddWithValue("$date", SqliteValueConverter.FormatDateOnly(localDate));
        command.Parameters.AddWithValue("$completed", (int)FocusStatus.Completed);
        command.Parameters.AddWithValue("$stopwatch", (int)FocusMode.Stopwatch);
        command.Parameters.AddWithValue("$pomodoro", (int)FocusMode.Pomodoro);
        command.Parameters.AddWithValue("$focus", (int)PomodoroKind.Focus);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        return await reader.ReadAsync(cancellationToken)
            ? new FocusDaySummary(reader.GetInt64(1), reader.GetInt32(0))
            : FocusDaySummary.Empty;
    }
    private static async Task InsertAsync(SqliteConnection c, SqliteTransaction tx, FocusSession value, CancellationToken ct) { await using var x=c.CreateCommand();x.Transaction=tx;x.CommandText="""INSERT INTO focus_sessions (id,task_id,mode,started_at_utc,active_segment_started_utc,ended_at_utc,target_end_at_utc,duration_seconds,status,pomodoro_kind,created_local_date,task_title_snapshot,quadrant_snapshot) VALUES ($id,$task,$mode,$started,$active,$ended,$target,$duration,$status,$kind,$date,$title,$quadrant);""";AddParameters(x,value);await x.ExecuteNonQueryAsync(ct); }
    private static async Task<FocusSession?> ReadOneAsync(SqliteConnection c, SqliteTransaction? tx, string sql, CancellationToken ct, params (string Name, object Value)[] values) { await using var x=c.CreateCommand();x.Transaction=tx;x.CommandText=sql;foreach(var (n,v) in values)x.Parameters.AddWithValue(n,v);await using var r=await x.ExecuteReaderAsync(ct);return await r.ReadAsync(ct)?Map(r):null; }
    private static async Task<(string Title, int? QuadrantId, bool IsCompleted)?> ReadTaskSnapshotAsync(SqliteConnection c, SqliteTransaction tx, long id, CancellationToken ct) { await using var x=c.CreateCommand();x.Transaction=tx;x.CommandText="SELECT title, quadrant_id, is_completed FROM tasks WHERE id=$id;";x.Parameters.AddWithValue("$id",id);await using var r=await x.ExecuteReaderAsync(ct);return await r.ReadAsync(ct)?(r.GetString(0),r[1] is DBNull?null:Convert.ToInt32(r[1]),r.GetInt32(2)!=0):null; }
    private static FocusSession Map(SqliteDataReader r)=>new(r.GetString(r.GetOrdinal("id")),Long(r,"task_id"),(FocusMode)r.GetInt32(r.GetOrdinal("mode")),SqliteValueConverter.ParseDateTimeOffset(r["started_at_utc"]),Time(r,"active_segment_started_utc"),Time(r,"ended_at_utc"),Time(r,"target_end_at_utc"),r.GetInt32(r.GetOrdinal("duration_seconds")),(FocusStatus)r.GetInt32(r.GetOrdinal("status")),r["pomodoro_kind"] is DBNull?null:(PomodoroKind)r.GetInt32(r.GetOrdinal("pomodoro_kind")),SqliteValueConverter.ParseDateOnly(r["created_local_date"]),Text(r,"task_title_snapshot"),Int(r,"quadrant_snapshot"));
    private static void AddParameters(SqliteCommand x,FocusSession value){x.Parameters.AddWithValue("$id",value.Id); x.Parameters.AddWithValue("$task",SqliteValueConverter.ToDbValue(value.TaskId)); x.Parameters.AddWithValue("$mode",(int)value.Mode); x.Parameters.AddWithValue("$started",SqliteValueConverter.FormatUtc(value.StartedAtUtc)); x.Parameters.AddWithValue("$active",SqliteValueConverter.ToDbValue(value.ActiveSegmentStartedAtUtc)); x.Parameters.AddWithValue("$ended",SqliteValueConverter.ToDbValue(value.EndedAtUtc)); x.Parameters.AddWithValue("$target",SqliteValueConverter.ToDbValue(value.TargetEndAtUtc)); x.Parameters.AddWithValue("$duration",value.DurationSeconds); x.Parameters.AddWithValue("$status",(int)value.Status); x.Parameters.AddWithValue("$kind",value.PomodoroKind is null ? DBNull.Value : (object)(int)value.PomodoroKind.Value); x.Parameters.AddWithValue("$date",SqliteValueConverter.FormatDateOnly(value.CreatedLocalDate)); x.Parameters.AddWithValue("$title",SqliteValueConverter.ToDbValue(value.TaskTitleSnapshot)); x.Parameters.AddWithValue("$quadrant",SqliteValueConverter.ToDbValue(value.QuadrantSnapshot));}
    private async Task<SqliteConnection> OpenAsync(CancellationToken ct){var c=connectionFactory.CreateConnection();await c.OpenAsync(ct);await SqliteDatabaseInitializer.ConfigureConnectionAsync(c,ct);return c;}
    private static long? Long(SqliteDataReader r,string n)=>r[n] is DBNull?null:Convert.ToInt64(r[n]); private static int? Int(SqliteDataReader r,string n)=>r[n] is DBNull?null:Convert.ToInt32(r[n]); private static string? Text(SqliteDataReader r,string n)=>r[n] is DBNull?null:Convert.ToString(r[n]); private static DateTimeOffset? Time(SqliteDataReader r,string n)=>r[n] is DBNull?null:SqliteValueConverter.ParseDateTimeOffset(r[n]);
}
