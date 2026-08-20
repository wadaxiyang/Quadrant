using System.Globalization;
using Microsoft.Data.Sqlite;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteTaskRepository : ITaskRepository
{
    private readonly SqliteConnectionFactory connectionFactory;

    public SqliteTaskRepository(SqliteConnectionFactory connectionFactory)
    {
        this.connectionFactory = connectionFactory ?? throw new ArgumentNullException(nameof(connectionFactory));
    }

    public async Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default) =>
        await GetManyAsync("SELECT * FROM tasks WHERE is_completed = 0 ORDER BY quadrant_id, created_at, id;", cancellationToken);

    public async Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default) =>
        await GetManyAsync("SELECT * FROM tasks WHERE is_completed = 1 ORDER BY completed_at DESC, id DESC;", cancellationToken);

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
            INSERT INTO tasks (title, quadrant_id, due_at, reminder_at, note, is_completed, completed_at, created_at, updated_at)
            VALUES ($title, $quadrant_id, $due_at, $reminder_at, $note, 0, NULL, $created_at, $updated_at);
            SELECT last_insert_rowid();
            """;
        AddTaskParameters(command, draft.Title, draft.QuadrantId, draft.DueAt, draft.ReminderAt, draft.Note, now, now);
        var id = Convert.ToInt64(await command.ExecuteScalarAsync(cancellationToken), CultureInfo.InvariantCulture);
        await transaction.CommitAsync(cancellationToken);
        return new TaskItem(id, draft.Title, draft.QuadrantId, draft.DueAt, draft.ReminderAt, draft.Note, false, null, now, now);
    }

    public async Task<TaskItem> UpdateAsync(TaskUpdate update, DateTimeOffset now, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            UPDATE tasks
            SET title = $title, quadrant_id = $quadrant_id, due_at = $due_at, reminder_at = $reminder_at,
                note = $note, updated_at = $updated_at
            WHERE id = $id;
            """;
        command.Parameters.AddWithValue("$id", update.Id);
        AddTaskParameters(command, update.Title, update.QuadrantId, update.DueAt, update.ReminderAt, update.Note, now, now, includeCreatedAt: false);
        if (await command.ExecuteNonQueryAsync(cancellationToken) == 0)
        {
            throw new InvalidOperationException($"Task {update.Id} was not found.");
        }

        return (await GetByIdAsync(update.Id, cancellationToken))!;
    }

    public async Task<TaskItem> SetCompletedAsync(long id, bool isCompleted, DateTimeOffset now, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            UPDATE tasks
            SET is_completed = $is_completed, completed_at = $completed_at, updated_at = $updated_at
            WHERE id = $id;
            """;
        command.Parameters.AddWithValue("$id", id);
        command.Parameters.AddWithValue("$is_completed", isCompleted ? 1 : 0);
        command.Parameters.AddWithValue("$completed_at", isCompleted ? SqliteValueConverter.Format(now) : DBNull.Value);
        command.Parameters.AddWithValue("$updated_at", SqliteValueConverter.Format(now));
        if (await command.ExecuteNonQueryAsync(cancellationToken) == 0)
        {
            throw new InvalidOperationException($"Task {id} was not found.");
        }

        return (await GetByIdAsync(id, cancellationToken))!;
    }

    public async Task DeleteAsync(long id, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "DELETE FROM tasks WHERE id = $id;";
        command.Parameters.AddWithValue("$id", id);
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    private async Task<IReadOnlyList<TaskItem>> GetManyAsync(string commandText, CancellationToken cancellationToken)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = commandText;
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var tasks = new List<TaskItem>();
        while (await reader.ReadAsync(cancellationToken))
        {
            tasks.Add(MapTask(reader));
        }

        return tasks;
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
            reader.GetInt32(reader.GetOrdinal("quadrant_id")),
            ReadNullableDateTimeOffset(reader, "due_at"),
            ReadNullableDateTimeOffset(reader, "reminder_at"),
            ReadNullableString(reader, "note"),
            reader.GetInt32(reader.GetOrdinal("is_completed")) != 0,
            ReadNullableDateTimeOffset(reader, "completed_at"),
            SqliteValueConverter.ParseDateTimeOffset(reader["created_at"]),
            SqliteValueConverter.ParseDateTimeOffset(reader["updated_at"]));

    private static DateTimeOffset? ReadNullableDateTimeOffset(SqliteDataReader reader, string name) =>
        reader[name] is DBNull ? null : SqliteValueConverter.ParseDateTimeOffset(reader[name]);

    private static string? ReadNullableString(SqliteDataReader reader, string name) =>
        reader[name] is DBNull ? null : Convert.ToString(reader[name], CultureInfo.InvariantCulture);

    private static void AddTaskParameters(
        SqliteCommand command,
        string title,
        int quadrantId,
        DateTimeOffset? dueAt,
        DateTimeOffset? reminderAt,
        string? note,
        DateTimeOffset createdAt,
        DateTimeOffset updatedAt,
        bool includeCreatedAt = true)
    {
        command.Parameters.AddWithValue("$title", title);
        command.Parameters.AddWithValue("$quadrant_id", quadrantId);
        command.Parameters.AddWithValue("$due_at", SqliteValueConverter.ToDbValue(dueAt));
        command.Parameters.AddWithValue("$reminder_at", SqliteValueConverter.ToDbValue(reminderAt));
        command.Parameters.AddWithValue("$note", SqliteValueConverter.ToDbValue(note));
        if (includeCreatedAt)
        {
            command.Parameters.AddWithValue("$created_at", SqliteValueConverter.Format(createdAt));
        }

        command.Parameters.AddWithValue("$updated_at", SqliteValueConverter.Format(updatedAt));
    }
}
