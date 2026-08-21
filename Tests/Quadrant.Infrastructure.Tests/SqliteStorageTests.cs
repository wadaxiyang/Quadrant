using System.Globalization;
using Microsoft.Data.Sqlite;
using Quadrant.Core.Models;
using Quadrant.Infrastructure.Storage;
using Quadrant.Infrastructure.Tests.Fixtures;
using Xunit;
using Xunit.Abstractions;

namespace Quadrant.Infrastructure.Tests;

public sealed class SqliteStorageTests
{
    private readonly ITestOutputHelper output;

    public SqliteStorageTests(ITestOutputHelper output) => this.output = output;

    [Fact]
    public async Task Fresh_database_migrates_with_default_quadrants_and_is_idempotent()
    {
        await using var database = await TestDatabase.CreateAsync();

        var quadrants = await database.Quadrants.GetAllAsync();
        await database.Initializer.InitializeAsync();
        var secondRead = await database.Quadrants.GetAllAsync();

        Assert.Equal(4, quadrants.Count);
        Assert.Equal(new[] { 1, 2, 3, 4 }, quadrants.Select(q => q.Id));
        Assert.Equal(quadrants, secondRead);
        Assert.Equal(4, await database.ReadSchemaVersionAsync());
    }

    [Fact]
    public async Task Task_crud_preserves_nullable_values_and_round_trips_DateTimeOffset()
    {
        await using var database = await TestDatabase.CreateAsync();
        var createdAt = new DateTimeOffset(2026, 8, 20, 9, 15, 30, 123, TimeSpan.FromHours(8));
        var dueAt = createdAt.AddDays(2);
        var reminderAt = createdAt.AddDays(1);

        var created = await database.Tasks.CreateAsync(
            new TaskDraft("Review 'important' note", 2, dueAt, reminderAt, "Plain note", new DateOnly(2026, 8, 25), 90, Quadrant.Core.Enums.RecurrenceKind.Monthly, 1, "series-1", 31),
            createdAt);
        var loaded = await database.Tasks.GetByIdAsync(created.Id);

        Assert.NotNull(loaded);
        Assert.Equal(createdAt, loaded!.CreatedAt);
        Assert.Equal(createdAt, loaded.UpdatedAt);
        Assert.Equal(dueAt, loaded.DueAt);
        Assert.Equal(reminderAt, loaded.ReminderAt);
        Assert.Equal("Review 'important' note", loaded.Title);
        Assert.Equal(new DateOnly(2026, 8, 25), loaded.PlannedDate);
        Assert.Equal(90, loaded.EstimatedMinutes);
        Assert.Equal(Quadrant.Core.Enums.RecurrenceKind.Monthly, loaded.RecurrenceKind);
        Assert.Equal("series-1", loaded.RecurrenceSeriesId);
        Assert.Equal(31, loaded.RecurrenceAnchorDay);

        var updatedAt = createdAt.AddHours(2);
        var updated = await database.Tasks.UpdateAsync(
            new Quadrant.Core.Models.TaskUpdate(created.Id, "Updated", 3, null, null, null),
            updatedAt);
        Assert.Equal(3, updated.QuadrantId);
        Assert.Null(updated.DueAt);
        Assert.Null(updated.ReminderAt);
        Assert.Null(updated.Note);
        Assert.Equal(updatedAt, updated.UpdatedAt);
    }

    [Fact]
    public async Task Today_candidates_include_active_due_or_old_plan_but_exclude_completed_and_future_plan_only()
    {
        await using var database = await TestDatabase.CreateAsync();
        var now = new DateTimeOffset(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8));
        var due = await database.Tasks.CreateAsync(new TaskDraft("Due", 1, DueAt: now.AddDays(10)), now);
        var oldPlan = await database.Tasks.CreateAsync(new TaskDraft("Old plan", 1, PlannedDate: new DateOnly(2026, 8, 20)), now);
        var futurePlan = await database.Tasks.CreateAsync(new TaskDraft("Future plan", 1, PlannedDate: new DateOnly(2026, 8, 22)), now);
        var completedDue = await database.Tasks.CreateAsync(new TaskDraft("Completed due", 1, DueAt: now), now);
        await database.Tasks.SetCompletedAsync(completedDue.Id, true, now.AddMinutes(1));

        var candidates = await ((Quadrant.Core.Interfaces.ITodayTaskRepository)database.Tasks)
            .GetTodayCandidatesAsync(new DateOnly(2026, 8, 21));

        Assert.Equal([due.Id, oldPlan.Id], candidates.Select(task => task.Id).OrderBy(id => id));
        Assert.DoesNotContain(candidates, task => task.Id == futurePlan.Id);
        Assert.DoesNotContain(candidates, task => task.Id == completedDue.Id);
    }

    [Fact]
    public async Task Complete_restore_delete_and_completed_order_work()
    {
        await using var database = await TestDatabase.CreateAsync();
        var first = await database.Tasks.CreateAsync(new TaskDraft("First", 1), DateTimeOffset.UtcNow);
        var second = await database.Tasks.CreateAsync(new TaskDraft("Second", 1), DateTimeOffset.UtcNow.AddMinutes(1));

        var completed = await database.Tasks.SetCompletedAsync(first.Id, true, DateTimeOffset.UtcNow.AddMinutes(2));
        Assert.True(completed.IsCompleted);
        Assert.NotNull(completed.CompletedAt);
        Assert.Single(await database.Tasks.GetCompletedAsync());

        var restored = await database.Tasks.SetCompletedAsync(first.Id, false, DateTimeOffset.UtcNow.AddMinutes(3));
        Assert.False(restored.IsCompleted);
        Assert.Null(restored.CompletedAt);
        Assert.Empty(await database.Tasks.GetCompletedAsync());

        await database.Tasks.DeleteAsync(second.Id);
        Assert.Null(await database.Tasks.GetByIdAsync(second.Id));
    }

    [Fact]
    public async Task Foreign_keys_are_enabled_for_each_connection()
    {
        await using var database = await TestDatabase.CreateAsync();

        await using var connection = database.Factory.CreateConnection();
        await connection.OpenAsync();
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
        await using var command = connection.CreateCommand();
        command.CommandText = "PRAGMA foreign_keys;";

        Assert.Equal(1L, await command.ExecuteScalarAsync());
    }

    [Fact]
    public async Task Settings_and_quadrants_are_saved_atomically()
    {
        await using var database = await TestDatabase.CreateAsync();
        var originalSettings = await database.Settings.GetAsync();
        var originalQuadrants = await database.Quadrants.GetAllAsync();
        var changedSettings = originalSettings with { Theme = "Dark", StartMinimized = true };
        var changedQuadrants = originalQuadrants
            .Select(quadrant => quadrant with { Name = $"Changed {quadrant.Id}" })
            .ToArray();

        await database.Settings.SaveAsync(changedSettings, changedQuadrants);

        Assert.Equal(changedSettings, await database.Settings.GetAsync());
        Assert.Equal(changedQuadrants, await database.Quadrants.GetAllAsync());

        var invalidQuadrants = changedQuadrants
            .Append(new QuadrantDefinition(99, "Missing", "Missing"))
            .ToArray();
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            database.Settings.SaveAsync(originalSettings, invalidQuadrants));

        Assert.Equal(changedSettings, await database.Settings.GetAsync());
        Assert.Equal(changedQuadrants, await database.Quadrants.GetAllAsync());
    }

    [Fact]
    public async Task One_thousand_active_tasks_load_within_a_measurable_baseline()
    {
        await using var database = await TestDatabase.CreateAsync();
        var seed = DateTimeOffset.UtcNow;
        for (var index = 0; index < 1000; index++)
        {
            await database.Tasks.CreateAsync(new TaskDraft($"Synthetic task {index}", index % 4 + 1), seed.AddSeconds(index));
        }

        var stopwatch = System.Diagnostics.Stopwatch.StartNew();
        var tasks = await database.Tasks.GetActiveAsync();
        stopwatch.Stop();

        output.WriteLine($"1000-task SQLite active load: {stopwatch.ElapsedMilliseconds} ms; rows={tasks.Count}");
        Assert.Equal(1000, tasks.Count);
    }

    [Fact]
    public async Task Deterministic_v1_schema2_fixture_opens_without_data_loss()
    {
        var directory = Path.Combine(Path.GetTempPath(), "QuadrantTests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        var databasePath = Path.Combine(directory, "v1-schema2.db");
        try
        {
            await V1Schema2Fixture.CreateAsync(databasePath);
            var factory = new SqliteConnectionFactory(databasePath, pooling: false);
            var initializer = new SqliteDatabaseInitializer(factory);
            await initializer.InitializeAsync();
            var repository = new SqliteTaskRepository(factory);

            var active = await repository.GetActiveAsync();
            var completed = await repository.GetCompletedAsync();

            Assert.Equal(4, await ReadSchemaVersionAsync(factory));
            Assert.Equal(2, active.Count);
            Assert.Single(completed);
            Assert.Contains(active, task => task.Id == 101 && task.Title == "中文活动任务" && task.DueAt is not null && task.ReminderAt is not null && task.Note == "含中文与提醒");
            Assert.Contains(active, task => task.Id == 103 && task.DueAt is null && task.ReminderAt is not null);
            Assert.Equal(102, completed[0].Id);
            Assert.Equal("Completed note", completed[0].Note);
        }
        finally
        {
            try
            {
                Directory.Delete(directory, recursive: true);
            }
            catch (DirectoryNotFoundException)
            {
            }
        }
    }

    [Fact]
    public async Task Inbox_task_round_trips_without_entering_the_home_active_query()
    {
        await using var database = await TestDatabase.CreateAsync();
        var created = await database.Tasks.CreateAsync(new TaskDraft("Inbox", null), DateTimeOffset.UtcNow);

        Assert.Null((await database.Tasks.GetByIdAsync(created.Id))!.QuadrantId);
        Assert.Empty(await database.Tasks.GetActiveAsync());
    }

    [Fact]
    public async Task Inbox_and_home_queries_are_exclusive_and_classification_preserves_metadata()
    {
        await using var database = await TestDatabase.CreateAsync();
        var now = new DateTimeOffset(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8));
        var inbox = await database.Tasks.CreateAsync(
            new TaskDraft("Inbox", null, now.AddDays(1), now.AddHours(2), "note", new DateOnly(2026, 8, 22), 30), now);
        var home = await database.Tasks.CreateAsync(new TaskDraft("Home", 1), now.AddMinutes(1));
        await database.Tasks.CompleteWithSnapshotAsync(inbox.Id, now.AddMinutes(2));

        Assert.Equal([home.Id], (await database.Tasks.GetActiveAsync()).Select(task => task.Id));
        Assert.Empty(await database.Tasks.GetInboxAsync());

        var reopened = await database.Tasks.ReopenWithSnapshotRevertedAsync(inbox.Id, now.AddMinutes(3));
        var assigned = await database.Tasks.AssignQuadrantAsync(inbox.Id, 3, now.AddMinutes(4));
        var unchanged = await database.Tasks.AssignQuadrantAsync(inbox.Id, 3, now.AddMinutes(5));
        var movedToInbox = await database.Tasks.MoveToInboxAsync(inbox.Id, now.AddMinutes(6));

        Assert.False(reopened.IsCompleted);
        Assert.Equal(3, assigned.QuadrantId);
        Assert.Equal(inbox.DueAt, assigned.DueAt);
        Assert.Equal(inbox.ReminderAt, assigned.ReminderAt);
        Assert.Equal(inbox.PlannedDate, assigned.PlannedDate);
        Assert.Equal(inbox.EstimatedMinutes, assigned.EstimatedMinutes);
        Assert.Equal(assigned.UpdatedAt, unchanged.UpdatedAt);
        Assert.Null(movedToInbox.QuadrantId);
        Assert.Equal([home.Id], (await database.Tasks.GetActiveAsync()).Select(task => task.Id));
        Assert.Equal([inbox.Id], (await database.Tasks.GetInboxAsync()).Select(task => task.Id));
        Assert.Equal([inbox.Id], (await database.Tasks.GetInboxAsync(limit: 1)).Select(task => task.Id));
    }

    [Theory]
    [InlineData(0)]
    [InlineData(5)]
    public async Task Inbox_repository_rejects_invalid_quadrants_and_completed_tasks(int quadrantId)
    {
        await using var database = await TestDatabase.CreateAsync();
        var inbox = await database.Tasks.CreateAsync(new TaskDraft("Inbox", null), DateTimeOffset.UtcNow);

        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() => database.Tasks.AssignQuadrantAsync(inbox.Id, quadrantId, DateTimeOffset.UtcNow));
        await Assert.ThrowsAsync<InvalidOperationException>(() => database.Tasks.AssignQuadrantAsync(999, 1, DateTimeOffset.UtcNow));
        await database.Tasks.CompleteWithSnapshotAsync(inbox.Id, DateTimeOffset.UtcNow);
        await Assert.ThrowsAsync<InvalidOperationException>(() => database.Tasks.MoveToInboxAsync(inbox.Id, DateTimeOffset.UtcNow));
    }

    [Fact]
    public async Task Completion_event_and_focus_session_round_trip_with_long_task_id()
    {
        await using var database = await TestDatabase.CreateAsync();
        var task = await database.Tasks.CreateAsync(new TaskDraft("History", 1), DateTimeOffset.UtcNow);
        var events = new SqliteCompletionEventRepository(database.Factory);
        var sessions = new SqliteFocusSessionRepository(database.Factory);
        var localDate = new DateOnly(2026, 8, 21);
        await events.CreateAsync(new Quadrant.Core.Models.CompletionEvent("event-1", task.Id, DateTimeOffset.UtcNow, localDate, 1, "History", null, null, null, false));
        Assert.NotNull(await sessions.CreateIfNoCurrentAsync(new Quadrant.Core.Models.FocusSession("session-1", task.Id, Quadrant.Core.Enums.FocusMode.Stopwatch, DateTimeOffset.UtcNow, null, null, null, 0, Quadrant.Core.Enums.FocusStatus.Paused, null, localDate, "History", 1)));

        Assert.Equal(task.Id, (await events.GetByIdAsync("event-1"))!.TaskId);
        Assert.Equal(task.Id, (await sessions.GetByIdAsync("session-1"))!.TaskId);
    }

    [Fact]
    public async Task Focus_repository_enforces_one_current_session_and_transition_status()
    {
        await using var database = await TestDatabase.CreateAsync();
        var task = await database.Tasks.CreateAsync(new TaskDraft("Focus", 1), DateTimeOffset.UtcNow);
        var sessions = new SqliteFocusSessionRepository(database.Factory);
        var start = DateTimeOffset.UtcNow;
        var running = new FocusSession("focus-running", task.Id, Quadrant.Core.Enums.FocusMode.Stopwatch, start, start, null, null, 0, Quadrant.Core.Enums.FocusStatus.Running, null, DateOnly.FromDateTime(start.Date), "Focus", 1);

        Assert.NotNull(await sessions.CreateIfNoCurrentAsync(running));
        Assert.Null(await sessions.CreateIfNoCurrentAsync(running with { Id = "second" }));
        var paused = await sessions.TransitionAsync(running with { Status = Quadrant.Core.Enums.FocusStatus.Paused, ActiveSegmentStartedAtUtc = null, DurationSeconds = 30 }, Quadrant.Core.Enums.FocusStatus.Running);
        Assert.NotNull(paused);
        Assert.Null(await sessions.TransitionAsync(running with { Status = Quadrant.Core.Enums.FocusStatus.Completed }, Quadrant.Core.Enums.FocusStatus.Running));
        await database.Tasks.DeleteAsync(task.Id);
        var stored = await sessions.GetByIdAsync(running.Id);
        Assert.Null(stored!.TaskId);
        Assert.Equal("Focus", stored.TaskTitleSnapshot);
    }

    [Fact]
    public async Task Atomic_completion_snapshots_before_state_and_is_idempotent()
    {
        await using var database = await TestDatabase.CreateAsync();
        var now = new DateTimeOffset(2026, 8, 21, 9, 30, 0, TimeSpan.FromHours(8));
        var task = await database.Tasks.CreateAsync(
            new TaskDraft("Snapshot", 2, now.AddHours(-1), null, "note", new DateOnly(2026, 8, 22), 45), now.AddDays(-1));
        var events = new SqliteCompletionEventRepository(database.Factory);

        var first = await database.Tasks.CompleteWithSnapshotAsync(task.Id, now);
        var second = await database.Tasks.CompleteWithSnapshotAsync(task.Id, now.AddMinutes(1));
        var snapshot = await events.GetByIdAsync(first.Event!.Id);

        Assert.True(first.Task.IsCompleted);
        Assert.Equal(now.ToUniversalTime(), first.Task.CompletedAt);
        Assert.False(first.WasAlreadyCompleted);
        Assert.NotNull(snapshot);
        Assert.Equal(task.Id, snapshot!.TaskId);
        Assert.Equal("Snapshot", snapshot.TaskTitleSnapshot);
        Assert.Equal(2, snapshot.QuadrantSnapshot);
        Assert.Equal(now.AddHours(-1).ToUniversalTime(), snapshot.DueAtUtcSnapshot);
        Assert.Equal(new DateOnly(2026, 8, 22), snapshot.PlannedDateSnapshot);
        Assert.Equal(45, snapshot.EstimatedMinutesSnapshot);
        Assert.True(snapshot.WasOverdue);
        Assert.Equal(new DateOnly(2026, 8, 21), snapshot.CompletedLocalDate);
        Assert.True(second.WasAlreadyCompleted);
        Assert.Null(second.Event);
        Assert.Equal(1, await ReadScalarAsync(database.Factory, "SELECT COUNT(*) FROM task_completion_events;"));
    }

    [Fact]
    public async Task Atomic_completion_creates_next_instance_once_and_keeps_inbox()
    {
        await using var database = await TestDatabase.CreateAsync();
        var now = new DateTimeOffset(2026, 8, 21, 9, 30, 0, TimeSpan.FromHours(8));
        var source = await database.Tasks.CreateAsync(new TaskDraft("Recurring inbox", null, now.AddDays(1), null, "note", null, 30,
            Quadrant.Core.Enums.RecurrenceKind.Daily, 1, "series", 22), now);
        var nextDraft = new TaskDraft("Recurring inbox", null, now.AddDays(2), null, "note", null, 30,
            Quadrant.Core.Enums.RecurrenceKind.Daily, 1, "series", 22);

        var first = await database.Tasks.CompleteWithSnapshotAsync(source.Id, now, _ => nextDraft);
        var second = await database.Tasks.CompleteWithSnapshotAsync(source.Id, now.AddMinutes(1), _ => nextDraft);

        Assert.NotNull(first.NextTask);
        Assert.Null(first.NextTask!.QuadrantId);
        Assert.True(first.Task.IsCompleted);
        Assert.True(second.WasAlreadyCompleted);
        Assert.Null(second.NextTask);
        Assert.Single(await database.Tasks.GetInboxAsync());
        Assert.Equal(1, await ReadScalarAsync(database.Factory, "SELECT COUNT(*) FROM task_completion_events;"));
        Assert.Equal(2, await ReadScalarAsync(database.Factory, "SELECT COUNT(*) FROM tasks;"));
    }

    [Fact]
    public async Task Reopen_reverts_latest_event_and_recompletion_creates_a_new_event()
    {
        await using var database = await TestDatabase.CreateAsync();
        var task = await database.Tasks.CreateAsync(new TaskDraft("Repeat", 1), DateTimeOffset.UtcNow);
        var events = new SqliteCompletionEventRepository(database.Factory);
        var first = await database.Tasks.CompleteWithSnapshotAsync(task.Id, DateTimeOffset.UtcNow);

        var reopened = await database.Tasks.ReopenWithSnapshotRevertedAsync(task.Id, DateTimeOffset.UtcNow.AddMinutes(1));
        var second = await database.Tasks.CompleteWithSnapshotAsync(task.Id, DateTimeOffset.UtcNow.AddMinutes(2));

        Assert.False(reopened.IsCompleted);
        Assert.NotNull((await events.GetByIdAsync(first.Event!.Id))!.RevertedAtUtc);
        Assert.NotEqual(first.Event.Id, second.Event!.Id);
        Assert.Equal(2, await ReadScalarAsync(database.Factory, "SELECT COUNT(*) FROM task_completion_events;"));
        Assert.Equal(1, await ReadScalarAsync(database.Factory, "SELECT COUNT(*) FROM task_completion_events WHERE reverted_at_utc IS NULL;"));
    }

    [Fact]
    public async Task Completion_event_insert_failure_rolls_back_task_update()
    {
        await using var database = await TestDatabase.CreateAsync();
        var task = await database.Tasks.CreateAsync(new TaskDraft("Atomic", 1), DateTimeOffset.UtcNow);
        var events = new SqliteCompletionEventRepository(database.Factory);
        await events.CreateAsync(new CompletionEvent("existing", task.Id, DateTimeOffset.UtcNow, DateOnly.FromDateTime(DateTime.Now), 1, "Atomic", null, null, null, false));

        await Assert.ThrowsAsync<SqliteException>(() => database.Tasks.CompleteWithSnapshotAsync(task.Id, DateTimeOffset.UtcNow));

        var loaded = await database.Tasks.GetByIdAsync(task.Id);
        Assert.False(loaded!.IsCompleted);
        Assert.Null(loaded.CompletedAt);
        Assert.Equal(1, await ReadScalarAsync(database.Factory, "SELECT COUNT(*) FROM task_completion_events;"));
    }

    [Fact]
    public async Task Missing_task_completion_does_not_create_an_event()
    {
        await using var database = await TestDatabase.CreateAsync();

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            database.Tasks.CompleteWithSnapshotAsync(999, DateTimeOffset.UtcNow));

        Assert.Equal("Task 999 was not found.", error.Message);
        Assert.Equal(0, await ReadScalarAsync(database.Factory, "SELECT COUNT(*) FROM task_completion_events;"));
    }

    [Fact]
    public async Task Legacy_completion_reopens_without_inventing_history_and_delete_preserves_snapshot()
    {
        await using var database = await TestDatabase.CreateAsync();
        var task = await database.Tasks.CreateAsync(new TaskDraft("Legacy", 1), DateTimeOffset.UtcNow);
        await ExecuteAsync(database.Factory, "UPDATE tasks SET is_completed = 1, completed_at = $completed;", ("$completed", SqliteValueConverter.FormatUtc(DateTimeOffset.UtcNow)));

        var reopened = await database.Tasks.ReopenWithSnapshotRevertedAsync(task.Id, DateTimeOffset.UtcNow.AddMinutes(1));
        Assert.False(reopened.IsCompleted);
        Assert.Equal(0, await ReadScalarAsync(database.Factory, "SELECT COUNT(*) FROM task_completion_events;"));

        var completed = await database.Tasks.CompleteWithSnapshotAsync(task.Id, DateTimeOffset.UtcNow.AddMinutes(2));
        var events = new SqliteCompletionEventRepository(database.Factory);
        await database.Tasks.DeleteAsync(task.Id);
        var snapshot = await events.GetByIdAsync(completed.Event!.Id);

        Assert.NotNull(snapshot);
        Assert.Null(snapshot!.TaskId);
        Assert.Equal("Legacy", snapshot.TaskTitleSnapshot);
    }

    [Fact]
    public async Task Future_schema_version_is_rejected_without_mutation()
    {
        await using var database = await TestDatabase.CreateAsync();
        await SetSchemaVersionAsync(database.Factory, 5);

        await Assert.ThrowsAsync<InvalidOperationException>(() => database.Initializer.InitializeAsync());
        Assert.Equal(5, await ReadSchemaVersionAsync(database.Factory));
    }

    [Fact]
    public async Task Failed_v2_to_v3_migration_rolls_back_to_intact_v2_schema()
    {
        var directory = Path.Combine(Path.GetTempPath(), "QuadrantTests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        var path = Path.Combine(directory, "v2.db");
        try
        {
            await V1Schema2Fixture.CreateAsync(path);
            var factory = new SqliteConnectionFactory(path, pooling: false);
            await using (var connection = factory.CreateConnection())
            {
                await connection.OpenAsync();
                await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
                await using var command = connection.CreateCommand();
                command.CommandText = "CREATE INDEX ix_tasks_active_planned ON settings(value);";
                await command.ExecuteNonQueryAsync();
            }

            var initializer = new SqliteDatabaseInitializer(factory);
            await Assert.ThrowsAsync<SqliteException>(() => initializer.InitializeAsync());

            Assert.Equal(2, await ReadSchemaVersionAsync(factory));
            Assert.Equal(3, await ReadScalarAsync(factory, "SELECT COUNT(*) FROM tasks;"));
            Assert.Equal(0, await ReadScalarAsync(factory, "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('tasks_v3', 'task_completion_events', 'focus_sessions');"));
        }
        finally
        {
            Directory.Delete(directory, recursive: true);
        }
    }

    private static async Task<int> ReadSchemaVersionAsync(SqliteConnectionFactory factory)
    {
        await using var connection = factory.CreateConnection();
        await connection.OpenAsync();
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
        await using var command = connection.CreateCommand();
        command.CommandText = "SELECT version FROM schema_version;";
        return Convert.ToInt32(await command.ExecuteScalarAsync(), CultureInfo.InvariantCulture);
    }

    private static async Task SetSchemaVersionAsync(SqliteConnectionFactory factory, int version)
    {
        await using var connection = factory.CreateConnection();
        await connection.OpenAsync();
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
        await using var command = connection.CreateCommand();
        command.CommandText = "UPDATE schema_version SET version = $version;";
        command.Parameters.AddWithValue("$version", version);
        await command.ExecuteNonQueryAsync();
    }

    private static async Task<int> ReadScalarAsync(SqliteConnectionFactory factory, string sql)
    {
        await using var connection = factory.CreateConnection();
        await connection.OpenAsync();
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
        await using var command = connection.CreateCommand();
        command.CommandText = sql;
        return Convert.ToInt32(await command.ExecuteScalarAsync(), CultureInfo.InvariantCulture);
    }

    private static async Task ExecuteAsync(SqliteConnectionFactory factory, string sql, params (string Name, object Value)[] parameters)
    {
        await using var connection = factory.CreateConnection();
        await connection.OpenAsync();
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
        await using var command = connection.CreateCommand();
        command.CommandText = sql;
        foreach (var (name, value) in parameters)
        {
            command.Parameters.AddWithValue(name, value);
        }

        await command.ExecuteNonQueryAsync();
    }

    private sealed class TestDatabase : IAsyncDisposable
    {
        private readonly string directory;

        private TestDatabase(string directory, SqliteConnectionFactory factory, SqliteDatabaseInitializer initializer)
        {
            this.directory = directory;
            Factory = factory;
            Initializer = initializer;
            Tasks = new SqliteTaskRepository(factory);
            Quadrants = new SqliteQuadrantRepository(factory);
            Settings = new SqliteSettingsRepository(factory);
        }

        public SqliteConnectionFactory Factory { get; }

        public SqliteDatabaseInitializer Initializer { get; }

        public SqliteTaskRepository Tasks { get; }

        public SqliteQuadrantRepository Quadrants { get; }

        public SqliteSettingsRepository Settings { get; }

        public static async Task<TestDatabase> CreateAsync()
        {
            var directory = Path.Combine(Path.GetTempPath(), "QuadrantTests", Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(directory);
            var path = Path.Combine(directory, "quadrant.db");
            var factory = new SqliteConnectionFactory(path, pooling: false);
            var initializer = new SqliteDatabaseInitializer(factory);
            await initializer.InitializeAsync();
            return new TestDatabase(directory, factory, initializer);
        }

        public async Task<int> ReadSchemaVersionAsync()
        {
            await using var connection = Factory.CreateConnection();
            await connection.OpenAsync();
            await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
            await using var command = connection.CreateCommand();
            command.CommandText = "SELECT version FROM schema_version;";
            return Convert.ToInt32(await command.ExecuteScalarAsync(), CultureInfo.InvariantCulture);
        }


        public ValueTask DisposeAsync()
        {
            try
            {
                Directory.Delete(directory, recursive: true);
            }
            catch (DirectoryNotFoundException)
            {
            }

            return ValueTask.CompletedTask;
        }
    }
}
