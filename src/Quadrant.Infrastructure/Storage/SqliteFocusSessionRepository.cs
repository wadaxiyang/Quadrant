using Microsoft.Data.Sqlite;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteFocusSessionRepository(SqliteConnectionFactory connectionFactory) : IFocusSessionRepository
{
    public async Task CreateAsync(FocusSession value, CancellationToken cancellationToken = default)
    {
        await using var c = await OpenAsync(cancellationToken); await using var x = c.CreateCommand();
        x.CommandText = """INSERT INTO focus_sessions (id,task_id,mode,started_at_utc,active_segment_started_utc,ended_at_utc,target_end_at_utc,duration_seconds,status,pomodoro_kind,created_local_date,task_title_snapshot,quadrant_snapshot) VALUES ($id,$task,$mode,$started,$active,$ended,$target,$duration,$status,$kind,$date,$title,$quadrant);""";
        x.Parameters.AddWithValue("$id",value.Id); x.Parameters.AddWithValue("$task",SqliteValueConverter.ToDbValue(value.TaskId)); x.Parameters.AddWithValue("$mode",(int)value.Mode); x.Parameters.AddWithValue("$started",SqliteValueConverter.FormatUtc(value.StartedAtUtc)); x.Parameters.AddWithValue("$active",SqliteValueConverter.ToDbValue(value.ActiveSegmentStartedAtUtc)); x.Parameters.AddWithValue("$ended",SqliteValueConverter.ToDbValue(value.EndedAtUtc)); x.Parameters.AddWithValue("$target",SqliteValueConverter.ToDbValue(value.TargetEndAtUtc)); x.Parameters.AddWithValue("$duration",value.DurationSeconds); x.Parameters.AddWithValue("$status",(int)value.Status); x.Parameters.AddWithValue("$kind",value.PomodoroKind is null ? DBNull.Value : (object)(int)value.PomodoroKind.Value); x.Parameters.AddWithValue("$date",SqliteValueConverter.FormatDateOnly(value.CreatedLocalDate)); x.Parameters.AddWithValue("$title",SqliteValueConverter.ToDbValue(value.TaskTitleSnapshot)); x.Parameters.AddWithValue("$quadrant",SqliteValueConverter.ToDbValue(value.QuadrantSnapshot));
        await x.ExecuteNonQueryAsync(cancellationToken);
    }

    public async Task<FocusSession?> GetByIdAsync(string id, CancellationToken cancellationToken = default)
    {
        await using var c=await OpenAsync(cancellationToken); await using var x=c.CreateCommand(); x.CommandText="SELECT * FROM focus_sessions WHERE id=$id;"; x.Parameters.AddWithValue("$id",id); await using var r=await x.ExecuteReaderAsync(cancellationToken); if(!await r.ReadAsync(cancellationToken)) return null;
        return new FocusSession(r.GetString(r.GetOrdinal("id")),Long(r,"task_id"),(FocusMode)r.GetInt32(r.GetOrdinal("mode")),SqliteValueConverter.ParseDateTimeOffset(r["started_at_utc"]),Time(r,"active_segment_started_utc"),Time(r,"ended_at_utc"),Time(r,"target_end_at_utc"),r.GetInt32(r.GetOrdinal("duration_seconds")),(FocusStatus)r.GetInt32(r.GetOrdinal("status")),r["pomodoro_kind"] is DBNull?null:(PomodoroKind)r.GetInt32(r.GetOrdinal("pomodoro_kind")),SqliteValueConverter.ParseDateOnly(r["created_local_date"]),Text(r,"task_title_snapshot"),Int(r,"quadrant_snapshot"));
    }
    private async Task<SqliteConnection> OpenAsync(CancellationToken ct){var c=connectionFactory.CreateConnection();await c.OpenAsync(ct);await SqliteDatabaseInitializer.ConfigureConnectionAsync(c,ct);return c;}
    private static long? Long(SqliteDataReader r,string n)=>r[n] is DBNull?null:Convert.ToInt64(r[n]); private static int? Int(SqliteDataReader r,string n)=>r[n] is DBNull?null:Convert.ToInt32(r[n]); private static string? Text(SqliteDataReader r,string n)=>r[n] is DBNull?null:Convert.ToString(r[n]); private static DateTimeOffset? Time(SqliteDataReader r,string n)=>r[n] is DBNull?null:SqliteValueConverter.ParseDateTimeOffset(r[n]);
}
