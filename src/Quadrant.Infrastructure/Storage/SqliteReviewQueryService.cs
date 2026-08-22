using System.Globalization;
using Microsoft.Data.Sqlite;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;

namespace Quadrant.Infrastructure.Storage;

/// <summary>SQLite aggregate queries for Review. History remains in SQLite; only aggregate DTOs leave this class.</summary>
public sealed class SqliteReviewQueryService(SqliteConnectionFactory connectionFactory, IClock clock) : IReviewQueryService
{
    private readonly ReviewRangeCalculator ranges = new(clock);

    public async Task<ReviewDashboard> GetDashboardAsync(ReviewRange range, DayOfWeek weekStart, int recentLimit = 20, CancellationToken cancellationToken = default)
    {
        if (recentLimit is < 1 or > 50) throw new ArgumentOutOfRangeException(nameof(recentLimit));
        var currentDates = ranges.GetRange(range);
        var previousDates = ranges.GetPreviousRange(range);
        var currentTask = GetSummaryForDatesAsync(currentDates, includeCurrentState: true, cancellationToken);
        var previousTask = previousDates is null
            ? Task.FromResult<ReviewSummary?>(null)
            : GetOptionalSummaryAsync(previousDates, cancellationToken);
        var completedTask = GetCompletedTrendAsync(range, weekStart, cancellationToken);
        var focusTask = GetFocusTrendAsync(range, weekStart, cancellationToken);
        var completedQuadrantsTask = GetCompletionByQuadrantAsync(range, cancellationToken);
        var focusQuadrantsTask = GetFocusByQuadrantAsync(range, cancellationToken);
        var focusSummaryTask = GetFocusSummaryAsync(currentDates, cancellationToken);
        var recentTask = GetRecentCompletedAsync(recentLimit, cancellationToken);
        await Task.WhenAll(currentTask, previousTask, completedTask, focusTask, completedQuadrantsTask, focusQuadrantsTask, focusSummaryTask, recentTask);
        return new ReviewDashboard(range, currentTask.Result, previousTask.Result, completedTask.Result, focusTask.Result,
            completedQuadrantsTask.Result, focusQuadrantsTask.Result, focusSummaryTask.Result, recentTask.Result);
    }

    public async Task<ReviewSummary> GetSummaryAsync(ReviewRange range, CancellationToken cancellationToken = default)
        => await GetSummaryForDatesAsync(ranges.GetRange(range), includeCurrentState: true, cancellationToken);

    private async Task<ReviewSummary> GetSummaryForDatesAsync(ReviewDateRange dates, bool includeCurrentState, CancellationToken cancellationToken)
    {
        await using var connection = await OpenAsync(cancellationToken);
        var completed = await ScalarIntAsync(connection, CompletionWhere("COUNT(*)"), dates, cancellationToken);
        var focus = await ReadFocusTotalsAsync(connection, dates, cancellationToken);
        var inbox = includeCurrentState ? await ScalarIntAsync(connection, "SELECT COUNT(*) FROM tasks WHERE is_completed = 0 AND quadrant_id IS NULL;", null, cancellationToken) : 0;
        var overdue = includeCurrentState ? await ScalarIntAsync(connection, "SELECT COUNT(*) FROM tasks WHERE is_completed = 0 AND due_at IS NOT NULL AND due_at < $now;", null, cancellationToken, ("$now", SqliteValueConverter.FormatUtc(clock.UtcNow))) : 0;
        return new ReviewSummary(completed, focus.Count, focus.Seconds, focus.Count == 0 ? 0 : (int)(focus.Seconds / focus.Count), focus.Count > 0, inbox, overdue);
    }

    private async Task<ReviewSummary?> GetOptionalSummaryAsync(ReviewDateRange dates, CancellationToken cancellationToken) =>
        await GetSummaryForDatesAsync(dates, includeCurrentState: false, cancellationToken);

    public Task<IReadOnlyList<DateBucketPoint>> GetCompletedTrendAsync(ReviewRange range, DayOfWeek weekStart, CancellationToken cancellationToken = default) =>
        GetTrendAsync("task_completion_events", "completed_local_date", "COUNT(*)", "reverted_at_utc IS NULL", range, weekStart, cancellationToken);

    public Task<IReadOnlyList<DateBucketPoint>> GetFocusTrendAsync(ReviewRange range, DayOfWeek weekStart, CancellationToken cancellationToken = default) =>
        GetTrendAsync("focus_sessions", "created_local_date", "COALESCE(SUM(duration_seconds), 0)", ProductiveFocusWhere, range, weekStart, cancellationToken);

    public Task<IReadOnlyList<QuadrantValue>> GetCompletionByQuadrantAsync(ReviewRange range, CancellationToken cancellationToken = default) =>
        GetQuadrantsAsync("task_completion_events", "completed_local_date", "quadrant_snapshot", "COUNT(*)", "reverted_at_utc IS NULL", range, cancellationToken);

    public Task<IReadOnlyList<QuadrantValue>> GetFocusByQuadrantAsync(ReviewRange range, CancellationToken cancellationToken = default) =>
        GetQuadrantsAsync("focus_sessions", "created_local_date", "quadrant_snapshot", "COALESCE(SUM(duration_seconds), 0)", ProductiveFocusWhere, range, cancellationToken);

    public async Task<IReadOnlyList<RecentCompletion>> GetRecentCompletedAsync(int limit = 20, CancellationToken cancellationToken = default)
    {
        if (limit is < 1 or > 50) throw new ArgumentOutOfRangeException(nameof(limit));
        await using var connection = await OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "SELECT id, completed_at_utc, completed_local_date, task_title_snapshot, quadrant_snapshot, was_overdue FROM task_completion_events WHERE reverted_at_utc IS NULL ORDER BY completed_at_utc DESC LIMIT $limit;";
        command.Parameters.AddWithValue("$limit", limit);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var values = new List<RecentCompletion>();
        while (await reader.ReadAsync(cancellationToken))
        {
            values.Add(new RecentCompletion(reader.GetString(0), SqliteValueConverter.ParseDateTimeOffset(reader[1]), SqliteValueConverter.ParseDateOnly(reader[2]), reader.GetString(3), reader[4] is DBNull ? null : reader.GetInt32(4), reader.GetInt32(5) != 0));
        }
        return values;
    }

    private async Task<IReadOnlyList<DateBucketPoint>> GetTrendAsync(string table, string dateColumn, string aggregate, string predicate, ReviewRange range, DayOfWeek weekStart, CancellationToken ct)
    {
        var dates = ranges.GetRange(range);
        await using var connection = await OpenAsync(ct);
        var days = await ReadDailyAsync(connection, table, dateColumn, aggregate, predicate, dates, ct);
        var bucket = await GetBucketKindAsync(connection, table, dateColumn, predicate, range, dates, ct);
        return Bucket(days, dates, bucket, weekStart);
    }

    private static async Task<List<(DateOnly Date, long Value)>> ReadDailyAsync(SqliteConnection c, string table, string dateColumn, string aggregate, string predicate, ReviewDateRange dates, CancellationToken ct)
    {
        await using var command = c.CreateCommand();
        command.CommandText = $"SELECT {dateColumn}, {aggregate} FROM {table} WHERE {predicate}{RangeClause(dateColumn, dates)} GROUP BY {dateColumn} ORDER BY {dateColumn};";
        AddRange(command, dates);
        await using var reader = await command.ExecuteReaderAsync(ct);
        var values = new List<(DateOnly, long)>();
        while (await reader.ReadAsync(ct)) values.Add((SqliteValueConverter.ParseDateOnly(reader[0]), Convert.ToInt64(reader[1], CultureInfo.InvariantCulture)));
        return values;
    }

    private static async Task<(int Count, long Seconds)> ReadFocusTotalsAsync(SqliteConnection c, ReviewDateRange dates, CancellationToken ct)
    {
        await using var command = c.CreateCommand();
        command.CommandText = $"SELECT COUNT(*), COALESCE(SUM(duration_seconds), 0) FROM focus_sessions WHERE {ProductiveFocusWhere}{RangeClause("created_local_date", dates)};";
        AddRange(command, dates);
        await using var reader = await command.ExecuteReaderAsync(ct);
        await reader.ReadAsync(ct);
        return (reader.GetInt32(0), reader.GetInt64(1));
    }

    private async Task<FocusReviewSummary> GetFocusSummaryAsync(ReviewDateRange dates, CancellationToken cancellationToken)
    {
        await using var connection = await OpenAsync(cancellationToken);
        var totals = await ReadFocusTotalsAsync(connection, dates, cancellationToken);
        long longest;
        await using (var command = connection.CreateCommand())
        {
            command.CommandText = $"SELECT COALESCE(MAX(duration_seconds), 0) FROM focus_sessions WHERE {ProductiveFocusWhere}{RangeClause("created_local_date", dates)};";
            AddRange(command, dates);
            longest = Convert.ToInt64(await command.ExecuteScalarAsync(cancellationToken), CultureInfo.InvariantCulture);
        }

        string? taskTitle = null; long taskSeconds = 0; var taskSessions = 0;
        await using (var command = connection.CreateCommand())
        {
            command.CommandText = $"SELECT task_title_snapshot, SUM(duration_seconds) AS seconds, COUNT(*) AS sessions FROM focus_sessions WHERE {ProductiveFocusWhere} AND task_title_snapshot IS NOT NULL AND TRIM(task_title_snapshot) <> ''{RangeClause("created_local_date", dates)} GROUP BY COALESCE(CAST(task_id AS TEXT), 'title:' || task_title_snapshot), task_title_snapshot ORDER BY seconds DESC, sessions DESC, task_title_snapshot LIMIT 1;";
            AddRange(command, dates);
            await using var reader = await command.ExecuteReaderAsync(cancellationToken);
            if (await reader.ReadAsync(cancellationToken)) { taskTitle = reader.GetString(0); taskSeconds = reader.GetInt64(1); taskSessions = reader.GetInt32(2); }
        }

        int? quadrantId = null; long quadrantSeconds = 0;
        await using (var command = connection.CreateCommand())
        {
            command.CommandText = $"SELECT quadrant_snapshot, SUM(duration_seconds) AS seconds FROM focus_sessions WHERE {ProductiveFocusWhere} AND quadrant_snapshot IS NOT NULL{RangeClause("created_local_date", dates)} GROUP BY quadrant_snapshot ORDER BY seconds DESC, quadrant_snapshot LIMIT 1;";
            AddRange(command, dates);
            await using var reader = await command.ExecuteReaderAsync(cancellationToken);
            if (await reader.ReadAsync(cancellationToken)) { quadrantId = reader.GetInt32(0); quadrantSeconds = reader.GetInt64(1); }
        }

        return new FocusReviewSummary(totals.Seconds, totals.Count, totals.Count == 0 ? 0 : totals.Seconds / totals.Count,
            longest, taskTitle, taskSeconds, taskSessions, quadrantId, quadrantSeconds);
    }

    private async Task<IReadOnlyList<QuadrantValue>> GetQuadrantsAsync(string table, string dateColumn, string quadrantColumn, string aggregate, string predicate, ReviewRange range, CancellationToken ct)
    {
        var dates = ranges.GetRange(range);
        await using var c = await OpenAsync(ct); await using var command = c.CreateCommand();
        command.CommandText = $"SELECT {quadrantColumn}, {aggregate} FROM {table} WHERE {predicate}{RangeClause(dateColumn, dates)} GROUP BY {quadrantColumn};";
        AddRange(command, dates); await using var reader = await command.ExecuteReaderAsync(ct);
        var values = new Dictionary<int, long>(); long unclassified = 0;
        while (await reader.ReadAsync(ct))
        {
            var value = Convert.ToInt64(reader[1], CultureInfo.InvariantCulture);
            if (reader[0] is DBNull) unclassified = value; else values[reader.GetInt32(0)] = value;
        }
        return [new QuadrantValue(1, "Q1", values.GetValueOrDefault(1)), new QuadrantValue(2, "Q2", values.GetValueOrDefault(2)), new QuadrantValue(3, "Q3", values.GetValueOrDefault(3)), new QuadrantValue(4, "Q4", values.GetValueOrDefault(4)), new QuadrantValue(null, table == "focus_sessions" ? "Unlinked" : "Inbox", unclassified)];
    }

    private static async Task<BucketKind> GetBucketKindAsync(SqliteConnection c, string table, string dateColumn, string predicate, ReviewRange range, ReviewDateRange dates, CancellationToken ct)
    {
        if (range is ReviewRange.SevenDays or ReviewRange.ThirtyDays) return BucketKind.Daily;
        if (range == ReviewRange.NinetyDays) return BucketKind.Weekly;
        await using var command = c.CreateCommand();
        command.CommandText = $"SELECT MIN({dateColumn}) FROM {table} WHERE {predicate}{RangeClause(dateColumn, dates)};";
        AddRange(command, dates); var result = await command.ExecuteScalarAsync(ct);
        return result is null or DBNull || dates.UpperExclusive.DayNumber - SqliteValueConverter.ParseDateOnly(result).DayNumber <= 90 ? BucketKind.Weekly : BucketKind.Monthly;
    }

    private static IReadOnlyList<DateBucketPoint> Bucket(List<(DateOnly Date, long Value)> days, ReviewDateRange dates, BucketKind kind, DayOfWeek weekStart)
    {
        var first = dates.LowerInclusive ?? (days.Count == 0 ? dates.UpperExclusive.AddDays(-1) : days[0].Date);
        var start = kind switch { BucketKind.Daily => first, BucketKind.Weekly => StartOfWeek(first, weekStart), _ => new DateOnly(first.Year, first.Month, 1) };
        var result = new Dictionary<DateOnly, long>();
        for (var value = start; value < dates.UpperExclusive; value = Next(value, kind)) result[value] = 0;
        foreach (var (date, value) in days) { var key = kind switch { BucketKind.Daily => date, BucketKind.Weekly => StartOfWeek(date, weekStart), _ => new DateOnly(date.Year, date.Month, 1) }; result[key] = result.GetValueOrDefault(key) + value; }
        return result.OrderBy(pair => pair.Key).Select(pair => new DateBucketPoint(pair.Key, pair.Key.ToString(kind == BucketKind.Monthly ? "yyyy-MM" : "yyyy-MM-dd", CultureInfo.InvariantCulture), pair.Value)).ToArray();
    }

    private static DateOnly Next(DateOnly date, BucketKind kind) => kind switch { BucketKind.Daily => date.AddDays(1), BucketKind.Weekly => date.AddDays(7), _ => date.AddMonths(1) };
    private static DateOnly StartOfWeek(DateOnly date, DayOfWeek weekStart) => date.AddDays(-((7 + (int)date.DayOfWeek - (int)weekStart) % 7));
    private static string CompletionWhere(string aggregate) => $"SELECT {aggregate} FROM task_completion_events WHERE reverted_at_utc IS NULL";
    private static string RangeClause(string column, ReviewDateRange dates) => (dates.LowerInclusive is null ? string.Empty : $" AND {column} >= $lower") + $" AND {column} < $upper";
    private static void AddRange(SqliteCommand command, ReviewDateRange dates) { if (dates.LowerInclusive is { } lower) command.Parameters.AddWithValue("$lower", SqliteValueConverter.FormatDateOnly(lower)); command.Parameters.AddWithValue("$upper", SqliteValueConverter.FormatDateOnly(dates.UpperExclusive)); }
    private static async Task<int> ScalarIntAsync(SqliteConnection c, string sql, ReviewDateRange? dates, CancellationToken ct, params (string Name, object Value)[] parameters) { await using var command = c.CreateCommand(); command.CommandText = dates is null ? sql : sql + RangeClause("completed_local_date", dates) + ";"; if (dates is not null) AddRange(command, dates); foreach (var (name, value) in parameters) command.Parameters.AddWithValue(name, value); return Convert.ToInt32(await command.ExecuteScalarAsync(ct), CultureInfo.InvariantCulture); }
    private async Task<SqliteConnection> OpenAsync(CancellationToken ct) { var c = connectionFactory.CreateConnection(); await c.OpenAsync(ct); await SqliteDatabaseInitializer.ConfigureConnectionAsync(c, ct); return c; }
    private const string ProductiveFocusWhere = "status = 3 AND (mode = 2 OR (mode = 1 AND pomodoro_kind = 1))";
    private enum BucketKind { Daily, Weekly, Monthly }
}
