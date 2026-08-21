using System.Globalization;
using Microsoft.Data.Sqlite;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteTaskRepository : ITaskRepository, ITodayTaskRepository
{
    private readonly SqliteConnectionFactory connectionFactory;

    public SqliteTaskRepository(SqliteConnectionFactory connectionFactory)
    {
        this.connectionFactory = connectionFactory ?? throw new ArgumentNullException(nameof(connectionFactory));
    }

    public async Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default) =>
        await GetManyAsync("SELECT * FROM tasks WHERE is_completed = 0 AND quadrant_id IS NOT NULL ORDER BY quadrant_id, created_at, id;", cancellationToken);

    public async Task<IReadOnlyList<TaskItem>> GetInboxAsync(int? limit = null, CancellationToken cancellationToken = default)
    {
        if (limit is <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(limit), "Inbox limit must be positive.");
        }

        var commandText = limit is null
            ? "SELECT * FROM tasks WHERE is_completed = 0 AND quadrant_id IS NULL ORDER BY created_at, id;"
            : "SELECT * FROM tasks WHERE is_completed = 0 AND quadrant_id IS NULL ORDER BY created_at, id LIMIT $limit;";
        return await GetManyAsync(commandText, limit, cancellationToken);
    }

    public async Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default) =>
        await GetManyAsync("SELECT * FROM tasks WHERE is_completed = 1 ORDER BY completed_at DESC, id DESC;", cancellationToken);

    public async Task<IReadOnlyList<TaskItem>> GetTodayCandidatesAsync(DateOnly localToday, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        // Due instants may still contain V1 offset text. Compare their parsed
        // DateTimeOffset values in the query service instead of asking SQLite to
        // infer a local date from the text. This query never reads completed/history.
        command.CommandText = """
            SELECT * FROM tasks
            WHERE is_completed = 0
              AND (due_at IS NOT NULL OR planned_date <= $local_today)
            ORDER BY due_at, planned_date, created_at, id;
            """;
        command.Parameters.AddWithValue("$local_today", SqliteValueConverter.FormatDateOnly(localToday));
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var tasks = new List<TaskItem>();
        while (await reader.ReadAsync(cancellationToken))
        {
            tasks.Add(MapTask(reader));
        }

        return tasks;
    }

    public async Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "SELECT * FROM tasks WHERE id = $id;";
        command.Parameters.AddWithValue("$id", id);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        return await reader.ReadAsync(cancellationToken) ? MapTask(reader) : null;
    }

    public async Task<TaskItem> CreateAsync(TaskDraft draft, DateTimeOffset now, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var transaction = connection.BeginTransaction();
        await using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = """
            INSERT INTO tasks (title, quadrant_id, due_at, reminder_at, note, is_completed, completed_at, created_at, updated_at,
                planned_date, estimated_minutes, recurrence_kind, recurrence_interval, recurrence_series_id, recurrence_anchor_day)
            VALUES ($title, $quadrant_id, $due_at, $reminder_at, $note, 0, NULL, $created_at, $updated_at,
                $planned_date, $estimated_minutes, $recurrence_kind, $recurrence_interval, $recurrence_series_id, $recurrence_anchor_day);
            SELECT last_insert_rowid();
            """;
        AddTaskParameters(command, draft, now, now);
        var id = Convert.ToInt64(await command.ExecuteScalarAsync(cancellationToken), CultureInfo.InvariantCulture);
        await transaction.CommitAsync(cancellationToken);
        return new TaskItem(id, draft.Title, draft.QuadrantId, draft.DueAt, draft.ReminderAt, draft.Note, false, null, now, now,
            draft.PlannedDate, draft.EstimatedMinutes, draft.RecurrenceKind, draft.RecurrenceInterval, draft.RecurrenceSeriesId, draft.RecurrenceAnchorDay);
    }

    public async Task<TaskItem> UpdateAsync(TaskUpdate update, DateTimeOffset now, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            UPDATE tasks
            SET title = $title, quadrant_id = $quadrant_id, due_at = $due_at, reminder_at = $reminder_at,
                note = $note, planned_date = $planned_date, estimated_minutes = $estimated_minutes,
                recurrence_kind = $recurrence_kind, recurrence_interval = $recurrence_interval,
                recurrence_series_id = $recurrence_series_id, recurrence_anchor_day = $recurrence_anchor_day, updated_at = $updated_at
            WHERE id = $id
            RETURNING *;
            """;
        command.Parameters.AddWithValue("$id", update.Id);
        AddTaskParameters(command, update, now, now, includeCreatedAt: false);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        if (!await reader.ReadAsync(cancellationToken))
        {
            throw new InvalidOperationException($"Task {update.Id} was not found.");
        }

        return MapTask(reader);
    }

    public async Task<TaskItem> AssignQuadrantAsync(long id, int quadrantId, DateTimeOffset now, CancellationToken cancellationToken = default)
    {
        if (quadrantId is < 1 or > 4)
        {
            throw new ArgumentOutOfRangeException(nameof(quadrantId), "Quadrant ID must be between 1 and 4.");
        }

        return await UpdateQuadrantAsync(id, quadrantId, now, cancellationToken);
    }

    public Task<TaskItem> MoveToInboxAsync(long id, DateTimeOffset now, CancellationToken cancellationToken = default) =>
        UpdateQuadrantAsync(id, null, now, cancellationToken);

    public async Task<TaskItem> SetCompletedAsync(long id, bool isCompleted, DateTimeOffset now, CancellationToken cancellationToken = default)
    {
        return isCompleted
            ? (await CompleteWithSnapshotAsync(id, now, cancellationToken)).Task
            : await ReopenWithSnapshotRevertedAsync(id, now, cancellationToken);
    }

    public async Task<CompletedTaskMutationResult> CompleteWithSnapshotAsync(long id, DateTimeOffset now, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var transaction = connection.BeginTransaction();
        var task = await ReadTaskAsync(connection, transaction, id, cancellationToken)
            ?? throw new InvalidOperationException($"Task {id} was not found.");
        if (task.IsCompleted)
        {
            await transaction.CommitAsync(cancellationToken);
            return new CompletedTaskMutationResult(task, null, true);
        }

        var completedUtc = now.ToUniversalTime();
        await using var update = connection.CreateCommand();
        update.Transaction = transaction;
        update.CommandText = "UPDATE tasks SET is_completed=1, completed_at=$completed, updated_at=$updated WHERE id=$id RETURNING *;";
        update.Parameters.AddWithValue("$id", id); update.Parameters.AddWithValue("$completed", SqliteValueConverter.FormatUtc(completedUtc)); update.Parameters.AddWithValue("$updated", SqliteValueConverter.FormatUtc(completedUtc));
        await using var reader = await update.ExecuteReaderAsync(cancellationToken);
        await reader.ReadAsync(cancellationToken);
        var completedTask = MapTask(reader);
        var completionEvent = new CompletionEvent(Guid.NewGuid().ToString("N"), id, completedUtc, DateOnly.FromDateTime(now.LocalDateTime), task.QuadrantId, task.Title, task.DueAt?.ToUniversalTime(), task.PlannedDate, task.EstimatedMinutes, task.DueAt is { } due && due < now);
        await reader.DisposeAsync();
        await InsertCompletionEventAsync(connection, transaction, completionEvent, cancellationToken);
        await transaction.CommitAsync(cancellationToken);
        return new CompletedTaskMutationResult(completedTask, completionEvent, false);
    }

    public async Task<TaskItem> ReopenWithSnapshotRevertedAsync(long id, DateTimeOffset now, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var transaction = connection.BeginTransaction();
        var task = await ReadTaskAsync(connection, transaction, id, cancellationToken)
            ?? throw new InvalidOperationException($"Task {id} was not found.");
        await using var update = connection.CreateCommand(); update.Transaction = transaction;
        update.CommandText = "UPDATE tasks SET is_completed=0, completed_at=NULL, updated_at=$updated WHERE id=$id RETURNING *;";
        update.Parameters.AddWithValue("$id", id); update.Parameters.AddWithValue("$updated", SqliteValueConverter.FormatUtc(now));
        await using var reader = await update.ExecuteReaderAsync(cancellationToken); await reader.ReadAsync(cancellationToken); var reopened = MapTask(reader); await reader.DisposeAsync();
        await SqliteDatabaseInitializer.ExecuteAsync(connection, transaction, "UPDATE task_completion_events SET reverted_at_utc=$now WHERE id=(SELECT id FROM task_completion_events WHERE task_id=$id AND reverted_at_utc IS NULL ORDER BY completed_at_utc DESC LIMIT 1);", cancellationToken, ("$now", SqliteValueConverter.FormatUtc(now)), ("$id", id));
        await transaction.CommitAsync(cancellationToken); return reopened;
    }

    public async Task DeleteAsync(long id, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "DELETE FROM tasks WHERE id = $id;";
        command.Parameters.AddWithValue("$id", id);
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    private async Task<TaskItem> UpdateQuadrantAsync(long id, int? quadrantId, DateTimeOffset now, CancellationToken cancellationToken)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var transaction = connection.BeginTransaction();
        var task = await ReadTaskAsync(connection, transaction, id, cancellationToken)
            ?? throw new InvalidOperationException($"Task {id} was not found.");
        if (task.IsCompleted)
        {
            throw new InvalidOperationException("Completed tasks cannot be classified.");
        }

        if (task.QuadrantId == quadrantId)
        {
            await transaction.CommitAsync(cancellationToken);
            return task;
        }

        await using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = "UPDATE tasks SET quadrant_id=$quadrant, updated_at=$updated WHERE id=$id RETURNING *;";
        command.Parameters.AddWithValue("$id", id);
        command.Parameters.AddWithValue("$quadrant", SqliteValueConverter.ToDbValue(quadrantId));
        command.Parameters.AddWithValue("$updated", SqliteValueConverter.FormatUtc(now));
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        await reader.ReadAsync(cancellationToken);
        var updated = MapTask(reader);
        await reader.DisposeAsync();
        await transaction.CommitAsync(cancellationToken);
        return updated;
    }

    private async Task<IReadOnlyList<TaskItem>> GetManyAsync(string commandText, int? limit, CancellationToken cancellationToken)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = commandText;
        if (limit is not null)
        {
            command.Parameters.AddWithValue("$limit", limit.Value);
        }
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var tasks = new List<TaskItem>();
        while (await reader.ReadAsync(cancellationToken))
        {
            tasks.Add(MapTask(reader));
        }

        return tasks;
    }

    private Task<IReadOnlyList<TaskItem>> GetManyAsync(string commandText, CancellationToken cancellationToken) =>
        GetManyAsync(commandText, null, cancellationToken);

    private static async Task<TaskItem?> ReadTaskAsync(SqliteConnection connection, SqliteTransaction transaction, long id, CancellationToken cancellationToken)
    {
        await using var command = connection.CreateCommand(); command.Transaction = transaction; command.CommandText = "SELECT * FROM tasks WHERE id=$id;"; command.Parameters.AddWithValue("$id", id);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken); return await reader.ReadAsync(cancellationToken) ? MapTask(reader) : null;
    }

    private static async Task InsertCompletionEventAsync(SqliteConnection connection, SqliteTransaction transaction, CompletionEvent value, CancellationToken cancellationToken)
    {
        await using var command = connection.CreateCommand(); command.Transaction = transaction;
        command.CommandText = """INSERT INTO task_completion_events (id,task_id,completed_at_utc,completed_local_date,quadrant_snapshot,task_title_snapshot,due_at_utc_snapshot,planned_date_snapshot,estimated_minutes_snapshot,was_overdue,reverted_at_utc) VALUES ($id,$task,$completed,$date,$quadrant,$title,$due,$planned,$estimate,$overdue,NULL);""";
        command.Parameters.AddWithValue("$id", value.Id); command.Parameters.AddWithValue("$task", value.TaskId!); command.Parameters.AddWithValue("$completed", SqliteValueConverter.FormatUtc(value.CompletedAtUtc)); command.Parameters.AddWithValue("$date", SqliteValueConverter.FormatDateOnly(value.CompletedLocalDate)); command.Parameters.AddWithValue("$quadrant", SqliteValueConverter.ToDbValue(value.QuadrantSnapshot)); command.Parameters.AddWithValue("$title", value.TaskTitleSnapshot); command.Parameters.AddWithValue("$due", SqliteValueConverter.ToDbValue(value.DueAtUtcSnapshot)); command.Parameters.AddWithValue("$planned", SqliteValueConverter.ToDbValue(value.PlannedDateSnapshot)); command.Parameters.AddWithValue("$estimate", SqliteValueConverter.ToDbValue(value.EstimatedMinutesSnapshot)); command.Parameters.AddWithValue("$overdue", value.WasOverdue ? 1 : 0);
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    private async Task<SqliteConnection> OpenConnectionAsync(CancellationToken cancellationToken)
    {
        var connection = connectionFactory.CreateConnection();
        try
        {
            await connection.OpenAsync(cancellationToken);
            await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, cancellationToken);
            return connection;
        }
        catch
        {
            await connection.DisposeAsync();
            throw;
        }
    }

    private static TaskItem MapTask(SqliteDataReader reader) =>
        new(
            reader.GetInt64(reader.GetOrdinal("id")),
            reader.GetString(reader.GetOrdinal("title")),
            ReadNullableInt32(reader, "quadrant_id"),
            ReadNullableDateTimeOffset(reader, "due_at"),
            ReadNullableDateTimeOffset(reader, "reminder_at"),
            ReadNullableString(reader, "note"),
            reader.GetInt32(reader.GetOrdinal("is_completed")) != 0,
            ReadNullableDateTimeOffset(reader, "completed_at"),
            SqliteValueConverter.ParseDateTimeOffset(reader["created_at"]),
            SqliteValueConverter.ParseDateTimeOffset(reader["updated_at"]),
            ReadNullableDateOnly(reader, "planned_date"),
            ReadNullableInt32(reader, "estimated_minutes"),
            (Quadrant.Core.Enums.RecurrenceKind)reader.GetInt32(reader.GetOrdinal("recurrence_kind")),
            reader.GetInt32(reader.GetOrdinal("recurrence_interval")),
            ReadNullableString(reader, "recurrence_series_id"),
            ReadNullableInt32(reader, "recurrence_anchor_day"));

    private static DateTimeOffset? ReadNullableDateTimeOffset(SqliteDataReader reader, string name) =>
        reader[name] is DBNull ? null : SqliteValueConverter.ParseDateTimeOffset(reader[name]);

    private static string? ReadNullableString(SqliteDataReader reader, string name) =>
        reader[name] is DBNull ? null : Convert.ToString(reader[name], CultureInfo.InvariantCulture);

    private static int? ReadNullableInt32(SqliteDataReader reader, string name) =>
        reader[name] is DBNull ? null : Convert.ToInt32(reader[name], CultureInfo.InvariantCulture);

    private static DateOnly? ReadNullableDateOnly(SqliteDataReader reader, string name) =>
        reader[name] is DBNull ? null : SqliteValueConverter.ParseDateOnly(reader[name]);

    private static void AddTaskParameters(
        SqliteCommand command,
        TaskDraft draft,
        DateTimeOffset createdAt,
        DateTimeOffset updatedAt,
        bool includeCreatedAt = true)
    {
        command.Parameters.AddWithValue("$title", draft.Title);
        command.Parameters.AddWithValue("$quadrant_id", SqliteValueConverter.ToDbValue(draft.QuadrantId));
        command.Parameters.AddWithValue("$due_at", SqliteValueConverter.ToDbValue(draft.DueAt));
        command.Parameters.AddWithValue("$reminder_at", SqliteValueConverter.ToDbValue(draft.ReminderAt));
        command.Parameters.AddWithValue("$note", SqliteValueConverter.ToDbValue(draft.Note));
        command.Parameters.AddWithValue("$planned_date", SqliteValueConverter.ToDbValue(draft.PlannedDate));
        command.Parameters.AddWithValue("$estimated_minutes", SqliteValueConverter.ToDbValue(draft.EstimatedMinutes));
        command.Parameters.AddWithValue("$recurrence_kind", (int)draft.RecurrenceKind);
        command.Parameters.AddWithValue("$recurrence_interval", draft.RecurrenceInterval);
        command.Parameters.AddWithValue("$recurrence_series_id", SqliteValueConverter.ToDbValue(draft.RecurrenceSeriesId));
        command.Parameters.AddWithValue("$recurrence_anchor_day", SqliteValueConverter.ToDbValue(draft.RecurrenceAnchorDay));
        if (includeCreatedAt)
        {
            command.Parameters.AddWithValue("$created_at", SqliteValueConverter.FormatUtc(createdAt));
        }

        command.Parameters.AddWithValue("$updated_at", SqliteValueConverter.FormatUtc(updatedAt));
    }

    private static void AddTaskParameters(SqliteCommand command, TaskUpdate update, DateTimeOffset createdAt, DateTimeOffset updatedAt, bool includeCreatedAt = true) =>
        AddTaskParameters(command, new TaskDraft(update.Title, update.QuadrantId, update.DueAt, update.ReminderAt, update.Note, update.PlannedDate,
            update.EstimatedMinutes, update.RecurrenceKind, update.RecurrenceInterval, update.RecurrenceSeriesId, update.RecurrenceAnchorDay), createdAt, updatedAt, includeCreatedAt);
}
