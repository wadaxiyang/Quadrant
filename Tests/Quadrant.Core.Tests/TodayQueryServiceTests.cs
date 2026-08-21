using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class TodayQueryServiceTests
{
    [Fact]
    public async Task Snapshot_applies_precedence_once_and_sums_only_unique_estimates()
    {
        var timeZone = TimeZoneInfo.CreateCustomTimeZone("Test", TimeSpan.FromHours(8), "Test", "Test");
        var now = new DateTimeOffset(2026, 8, 21, 12, 0, 0, TimeSpan.FromHours(8));
        var service = new TodayQueryService(new FakeRepository(
        [
            CreateTask(1, due: now.AddHours(-1), planned: new DateOnly(2026, 8, 21), estimate: 30),
            CreateTask(2, due: now.AddHours(2), planned: new DateOnly(2026, 8, 21), estimate: 40),
            CreateTask(3, due: now.AddHours(3), planned: new DateOnly(2026, 8, 20), estimate: 50),
            CreateTask(4, planned: new DateOnly(2026, 8, 20), estimate: 60),
            CreateTask(5, due: now.AddHours(4), estimate: 70),
            CreateTask(6, planned: new DateOnly(2026, 8, 21), estimate: null, quadrantId: null)
        ]), new FakeClock(now, timeZone));

        var snapshot = await service.GetSnapshotAsync();

        Assert.Equal([1L], snapshot.Overdue.Select(task => task.Id));
        Assert.Equal([2L, 6L], snapshot.PlannedToday.Select(task => task.Id));
        Assert.Equal([3L, 5L], snapshot.DueToday.Select(task => task.Id));
        Assert.Equal([4L], snapshot.NeedsReschedule.Select(task => task.Id));
        Assert.Equal(6, snapshot.UniqueTaskCount);
        Assert.Equal(250, snapshot.EstimatedMinutesTotal);
        Assert.Equal(0, snapshot.FocusedSecondsToday);
    }

    [Fact]
    public async Task Snapshot_treats_due_equal_to_now_as_due_today_and_uses_timezone_date()
    {
        var timeZone = TimeZoneInfo.CreateCustomTimeZone("West", TimeSpan.FromHours(-7), "West", "West");
        var now = new DateTimeOffset(2026, 8, 21, 0, 30, 0, TimeSpan.FromHours(-7));
        var dueAtNow = now;
        var service = new TodayQueryService(new FakeRepository([CreateTask(1, due: dueAtNow)]), new FakeClock(now, timeZone));

        var snapshot = await service.GetSnapshotAsync();

        Assert.Empty(snapshot.Overdue);
        Assert.Equal([1L], snapshot.DueToday.Select(task => task.Id));
    }

    private static TaskItem CreateTask(long id, DateTimeOffset? due = null, DateOnly? planned = null, int? estimate = null, int? quadrantId = 1) =>
        new(id, $"Task {id}", quadrantId, due, null, null, false, null,
            new DateTimeOffset(2026, 8, 1, 9, 0, 0, TimeSpan.Zero).AddMinutes(id),
            new DateTimeOffset(2026, 8, 1, 9, 0, 0, TimeSpan.Zero), planned, estimate);

    private sealed class FakeRepository(IReadOnlyList<TaskItem> tasks) : ITodayTaskRepository
    {
        public DateOnly? RequestedDate { get; private set; }
        public Task<IReadOnlyList<TaskItem>> GetTodayCandidatesAsync(DateOnly localToday, CancellationToken cancellationToken = default)
        {
            RequestedDate = localToday;
            return Task.FromResult(tasks);
        }
    }

    private sealed class FakeClock(DateTimeOffset now, TimeZoneInfo timeZone) : IClock
    {
        public DateTimeOffset UtcNow => now.ToUniversalTime();
        public DateTimeOffset LocalNow => now;
        public DateOnly LocalDate => DateOnly.FromDateTime(now.Date);
        public TimeZoneInfo LocalTimeZone => timeZone;
        public long GetTimestamp() => 0;
        public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }
}
