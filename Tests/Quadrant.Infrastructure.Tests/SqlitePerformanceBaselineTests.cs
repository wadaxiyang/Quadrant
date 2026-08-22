using System.Diagnostics;
using Microsoft.Data.Sqlite;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Infrastructure.Storage;
using Xunit;
using Xunit.Abstractions;

namespace Quadrant.Infrastructure.Tests;

public sealed class SqlitePerformanceBaselineTests(ITestOutputHelper output)
{
    [Fact]
    public async Task Large_fixture_queries_are_bounded_and_complete_within_soft_gate()
    {
        var directory = Path.Combine(Path.GetTempPath(), "QuadrantPerformanceTests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        try
        {
            var factory = new SqliteConnectionFactory(Path.Combine(directory, "performance.db"), false);
            await new SqliteDatabaseInitializer(factory).InitializeAsync();
            await SeedAsync(factory);
            var tasks = new SqliteTaskRepository(factory);
            var review = new SqliteReviewQueryService(factory, new FixedClock());

            var active = await MeasureAsync("active-1000", () => tasks.GetActiveAsync());
            var inbox = await MeasureAsync("inbox-1000", () => tasks.GetInboxAsync());
            var dashboard = await MeasureAsync("review-all-time-3650-history", () =>
                review.GetDashboardAsync(ReviewRange.AllTime, DayOfWeek.Monday));

            Assert.Equal(1000, active.Value.Count);
            Assert.Equal(1000, inbox.Value.Count);
            Assert.Equal(20, dashboard.Value.RecentCompleted.Count);
            Assert.InRange(dashboard.Value.CompletedActivity.Count, 1, 72);
            Assert.InRange(dashboard.Value.FocusActivity.Count, 1, 72);
            Assert.True(active.Elapsed < TimeSpan.FromSeconds(5), $"Active query took {active.Elapsed}.");
            Assert.True(inbox.Elapsed < TimeSpan.FromSeconds(5), $"Inbox query took {inbox.Elapsed}.");
            Assert.True(dashboard.Elapsed < TimeSpan.FromSeconds(5), $"Review query took {dashboard.Elapsed}.");
        }
        finally
        {
            if (Directory.Exists(directory)) Directory.Delete(directory, true);
        }
    }

    private async Task<(T Value, TimeSpan Elapsed)> MeasureAsync<T>(string name, Func<Task<T>> operation)
    {
        var stopwatch = Stopwatch.StartNew();
        var value = await operation();
        stopwatch.Stop();
        output.WriteLine("{0}: {1:F1} ms", name, stopwatch.Elapsed.TotalMilliseconds);
        return (value, stopwatch.Elapsed);
    }

    private static async Task SeedAsync(SqliteConnectionFactory factory)
    {
        await using var connection = factory.CreateConnection();
        await connection.OpenAsync();
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
        await using var transaction = connection.BeginTransaction();
        await using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = """
            WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 1000)
            INSERT INTO tasks (title, quadrant_id, created_at, updated_at)
            SELECT 'Active ' || x, ((x - 1) % 4) + 1, '2026-08-22T00:00:00.0000000+00:00', '2026-08-22T00:00:00.0000000+00:00' FROM n;

            WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 1000)
            INSERT INTO tasks (title, quadrant_id, created_at, updated_at)
            SELECT 'Inbox ' || x, NULL, '2026-08-22T00:00:00.0000000+00:00', '2026-08-22T00:00:00.0000000+00:00' FROM n;

            WITH RECURSIVE n(x) AS (SELECT 0 UNION ALL SELECT x + 1 FROM n WHERE x < 1824)
            INSERT INTO task_completion_events
                (id, completed_at_utc, completed_local_date, quadrant_snapshot, task_title_snapshot, was_overdue)
            SELECT 'completion-' || x, datetime('2021-08-23', '+' || x || ' days'),
                date('2021-08-23', '+' || x || ' days'), (x % 4) + 1, 'Completed ' || x, x % 2 FROM n;

            WITH RECURSIVE n(x) AS (SELECT 0 UNION ALL SELECT x + 1 FROM n WHERE x < 1824)
            INSERT INTO focus_sessions
                (id, mode, started_at_utc, ended_at_utc, duration_seconds, status, pomodoro_kind,
                 created_local_date, task_title_snapshot, quadrant_snapshot)
            SELECT 'focus-' || x, CASE WHEN x % 2 = 0 THEN 1 ELSE 2 END,
                datetime('2021-08-23', '+' || x || ' days'), datetime('2021-08-23', '+' || x || ' days', '+25 minutes'),
                1500, 3, CASE WHEN x % 2 = 0 THEN 1 ELSE NULL END,
                date('2021-08-23', '+' || x || ' days'), 'Focus ' || (x % 25), (x % 4) + 1 FROM n;
            """;
        await command.ExecuteNonQueryAsync();
        await transaction.CommitAsync();
    }

    private sealed class FixedClock : IClock
    {
        private static readonly DateTimeOffset Now = new(2026, 8, 22, 12, 0, 0, TimeSpan.FromHours(8));
        public DateTimeOffset UtcNow => Now.ToUniversalTime();
        public DateTimeOffset LocalNow => Now;
        public DateOnly LocalDate => new(2026, 8, 22);
        public TimeZoneInfo LocalTimeZone => TimeZoneInfo.Utc;
        public long GetTimestamp() => 0;
        public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }
}
