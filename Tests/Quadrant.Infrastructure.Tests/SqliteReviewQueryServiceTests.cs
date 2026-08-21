using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Infrastructure.Storage;
using Microsoft.Data.Sqlite;
using Xunit;

namespace Quadrant.Infrastructure.Tests;

public sealed class SqliteReviewQueryServiceTests
{
    [Fact]
    public async Task Aggregates_snapshots_and_excludes_reverted_break_and_out_of_range_history()
    {
        await using var database = await ReviewDatabase.CreateAsync();
        var now = new DateTimeOffset(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8));
        var events = new SqliteCompletionEventRepository(database.Factory);
        var sessions = new SqliteFocusSessionRepository(database.Factory);
        var active = await database.Tasks.CreateAsync(new TaskDraft("Overdue", 1, now.AddDays(-1)), now.AddDays(-2));
        await database.Tasks.CreateAsync(new TaskDraft("Inbox", null), now);
        await events.CreateAsync(new CompletionEvent("in-range", null, now, new DateOnly(2026, 8, 15), 1, "Snapshot title", null, null, null, false));
        await events.CreateAsync(new CompletionEvent("inbox", null, now.AddMinutes(-1), new DateOnly(2026, 8, 21), null, "Deleted task", null, null, null, false));
        await events.CreateAsync(new CompletionEvent("reverted", null, now, new DateOnly(2026, 8, 20), 2, "Reverted", null, null, null, false, now));
        await events.CreateAsync(new CompletionEvent("old", null, now, new DateOnly(2026, 8, 14), 3, "Old", null, null, null, false));
        await sessions.CreateIfNoCurrentAsync(Session("stopwatch", FocusMode.Stopwatch, FocusStatus.Completed, null, 2, 120, new DateOnly(2026, 8, 21)));
        await sessions.CreateIfNoCurrentAsync(Session("pomodoro-focus", FocusMode.Pomodoro, FocusStatus.Completed, PomodoroKind.Focus, null, 60, new DateOnly(2026, 8, 20)));
        await sessions.CreateIfNoCurrentAsync(Session("break", FocusMode.Pomodoro, FocusStatus.Completed, PomodoroKind.ShortBreak, null, 999, new DateOnly(2026, 8, 21)));
        await sessions.CreateIfNoCurrentAsync(Session("cancelled", FocusMode.Stopwatch, FocusStatus.Cancelled, null, 1, 999, new DateOnly(2026, 8, 21)));
        var service = new SqliteReviewQueryService(database.Factory, new FixedClock(now));

        var summary = await service.GetSummaryAsync(ReviewRange.SevenDays);
        var completed = await service.GetCompletedTrendAsync(ReviewRange.SevenDays, DayOfWeek.Monday);
        var focus = await service.GetFocusByQuadrantAsync(ReviewRange.SevenDays);
        var recent = await service.GetRecentCompletedAsync();

        Assert.Equal(2, summary.CompletedTaskCount);
        Assert.Equal(2, summary.ProductiveFocusSessionCount);
        Assert.Equal(180, summary.TotalFocusSeconds);
        Assert.Equal(90, summary.AverageFocusSeconds);
        Assert.True(summary.HasFocusData);
        Assert.Equal(1, summary.CurrentInboxCount);
        Assert.Equal(1, summary.CurrentOverdueCount);
        Assert.Equal(7, completed.Count);
        Assert.Equal(1, completed.Single(point => point.StartDate == new DateOnly(2026, 8, 15)).Value);
        Assert.Equal(0, completed.Single(point => point.StartDate == new DateOnly(2026, 8, 20)).Value);
        Assert.Equal(120, focus.Single(value => value.QuadrantId == 2).Value);
        Assert.Equal(60, focus.Single(value => value.QuadrantId is null).Value);
        Assert.Equal(new[] { "in-range", "inbox", "old" }, recent.Select(value => value.EventId).OrderBy(id => id));
        Assert.NotNull(await database.Tasks.GetByIdAsync(active.Id));
        Assert.Contains("ix_completion_local_date_active", await ExplainAsync(database.Factory, "SELECT COUNT(*) FROM task_completion_events WHERE reverted_at_utc IS NULL AND completed_local_date >= $lower AND completed_local_date < $upper;"));
        Assert.Contains("ix_focus_review_local_date_status", await ExplainAsync(database.Factory, "SELECT COUNT(*) FROM focus_sessions WHERE status = 3 AND created_local_date >= $lower AND created_local_date < $upper;"));
    }

    [Fact]
    public async Task Recent_limit_is_bounded_and_cancellation_is_observed()
    {
        await using var database = await ReviewDatabase.CreateAsync();
        var service = new SqliteReviewQueryService(database.Factory, new FixedClock(DateTimeOffset.UtcNow));
        var summary = await service.GetSummaryAsync(ReviewRange.SevenDays);
        Assert.Equal(0, summary.TotalFocusSeconds);
        Assert.Equal(0, summary.AverageFocusSeconds);
        Assert.False(summary.HasFocusData);
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() => service.GetRecentCompletedAsync(51));
        using var cancelled = new CancellationTokenSource(); cancelled.Cancel();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => service.GetSummaryAsync(ReviewRange.AllTime, cancelled.Token));
    }

    private static FocusSession Session(string id, FocusMode mode, FocusStatus status, PomodoroKind? kind, int? quadrant, int seconds, DateOnly date) =>
        new(id, null, mode, DateTimeOffset.UtcNow, null, DateTimeOffset.UtcNow, null, seconds, status, kind, date, "Snapshot", quadrant);

    private static async Task<string> ExplainAsync(SqliteConnectionFactory factory, string sql)
    {
        await using var connection = factory.CreateConnection(); await connection.OpenAsync();
        await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, default);
        await using var command = connection.CreateCommand(); command.CommandText = "EXPLAIN QUERY PLAN " + sql;
        command.Parameters.AddWithValue("$lower", "2026-08-15"); command.Parameters.AddWithValue("$upper", "2026-08-22");
        await using var reader = await command.ExecuteReaderAsync(); var details = new List<string>();
        while (await reader.ReadAsync()) details.Add(reader.GetString(3));
        return string.Join(Environment.NewLine, details);
    }

    private sealed class FixedClock(DateTimeOffset localNow) : IClock
    {
        public DateTimeOffset UtcNow => localNow.ToUniversalTime(); public DateTimeOffset LocalNow => localNow;
        public DateOnly LocalDate => DateOnly.FromDateTime(localNow.Date); public TimeZoneInfo LocalTimeZone => TimeZoneInfo.Utc;
        public long GetTimestamp() => 0; public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }

    private sealed class ReviewDatabase : IAsyncDisposable
    {
        private readonly string directory; private ReviewDatabase(string directory, SqliteConnectionFactory factory, SqliteTaskRepository tasks) { this.directory = directory; Factory = factory; Tasks = tasks; }
        public SqliteConnectionFactory Factory { get; } public SqliteTaskRepository Tasks { get; }
        public static async Task<ReviewDatabase> CreateAsync() { var directory = Path.Combine(Path.GetTempPath(), "QuadrantReviewTests", Guid.NewGuid().ToString("N")); Directory.CreateDirectory(directory); var factory = new SqliteConnectionFactory(Path.Combine(directory, "review.db"), false); await new SqliteDatabaseInitializer(factory).InitializeAsync(); return new ReviewDatabase(directory, factory, new SqliteTaskRepository(factory)); }
        public ValueTask DisposeAsync() { if (Directory.Exists(directory)) Directory.Delete(directory, true); return ValueTask.CompletedTask; }
    }
}
