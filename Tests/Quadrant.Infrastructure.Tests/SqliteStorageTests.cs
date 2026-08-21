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
        Assert.Equal(2, await database.ReadSchemaVersionAsync());
    }

    [Fact]
    public async Task Task_crud_preserves_nullable_values_and_round_trips_DateTimeOffset()
    {
        await using var database = await TestDatabase.CreateAsync();
        var createdAt = new DateTimeOffset(2026, 8, 20, 9, 15, 30, 123, TimeSpan.FromHours(8));
        var dueAt = createdAt.AddDays(2);
        var reminderAt = createdAt.AddDays(1);

        var created = await database.Tasks.CreateAsync(
            new TaskDraft("Review 'important' note", 2, dueAt, reminderAt, "Plain note"),
            createdAt);
        var loaded = await database.Tasks.GetByIdAsync(created.Id);

        Assert.NotNull(loaded);
        Assert.Equal(createdAt, loaded!.CreatedAt);
        Assert.Equal(createdAt, loaded.UpdatedAt);
        Assert.Equal(dueAt, loaded.DueAt);
        Assert.Equal(reminderAt, loaded.ReminderAt);
        Assert.Equal("Review 'important' note", loaded.Title);

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
