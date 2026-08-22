using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class PomodoroTimerServiceTests
{
    [Fact]
    public async Task Pause_stop_and_cancel_dispose_one_shot_schedules_and_ignore_stale_callbacks()
    {
        var clock = new Clock();
        var sessions = new Sessions(clock);
        var scheduler = new Scheduler();
        var timer = new PomodoroTimerService(sessions, clock, scheduler);
        var settings = new PomodoroSettings();

        await timer.StartAsync(null, PomodoroKind.Focus, settings);
        var first = Assert.Single(scheduler.Entries);
        await timer.PauseAsync();
        Assert.True(first.IsDisposed);

        await timer.ResumeAsync();
        var second = scheduler.Entries[1];
        await timer.StopAsync();
        Assert.True(second.IsDisposed);
        Assert.Equal(1, sessions.CompleteCalls);

        await timer.StartAsync(null, PomodoroKind.Focus, settings);
        var stale = scheduler.Entries[2];
        await timer.CancelAsync();
        Assert.True(stale.IsDisposed);
        await stale.Callback();
        Assert.Equal(1, sessions.CompleteCalls);
        Assert.Null(timer.Current);
    }

    private sealed class Scheduler : IFocusCompletionScheduler
    {
        public List<Entry> Entries { get; } = [];

        public IDisposable Schedule(DateTimeOffset dueAtUtc, Func<Task> callback)
        {
            var entry = new Entry(callback);
            Entries.Add(entry);
            return entry;
        }
    }

    private sealed class Entry(Func<Task> callback) : IDisposable
    {
        public Func<Task> Callback { get; } = callback;
        public bool IsDisposed { get; private set; }
        public void Dispose() => IsDisposed = true;
    }

    private sealed class Clock : IClock
    {
        public DateTimeOffset UtcNow { get; } = new(2026, 8, 22, 4, 0, 0, TimeSpan.Zero);
        public DateTimeOffset LocalNow => UtcNow;
        public DateOnly LocalDate => new(2026, 8, 22);
        public TimeZoneInfo LocalTimeZone => TimeZoneInfo.Utc;
        public long GetTimestamp() => 0;
        public TimeSpan GetElapsedTime(long startingTimestamp, long endingTimestamp) => TimeSpan.Zero;
    }

    private sealed class Sessions(Clock clock) : IFocusSessionService
    {
        private FocusSession? current;
        private int nextId;
        public int CompleteCalls { get; private set; }

        public Task<FocusSession> StartAsync(FocusSessionStartRequest request, CancellationToken cancellationToken = default)
        {
            current = new FocusSession(
                (++nextId).ToString(), request.TaskId, request.Mode, clock.UtcNow, clock.UtcNow,
                null, request.TargetEndAtUtc, 0, FocusStatus.Running, request.PomodoroKind,
                clock.LocalDate, null, null);
            return Task.FromResult(current);
        }

        public Task<FocusSession> PauseAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default) =>
            Task.FromResult(current = current! with { Status = FocusStatus.Paused, DurationSeconds = durationSeconds, ActiveSegmentStartedAtUtc = null });

        public Task<FocusSession> ResumeAsync(string id, DateTimeOffset at, CancellationToken cancellationToken = default) =>
            Task.FromResult(current = current! with { Status = FocusStatus.Running, ActiveSegmentStartedAtUtc = at });

        public Task<FocusSession> ResumeAsync(string id, DateTimeOffset at, DateTimeOffset? targetEndAtUtc, CancellationToken cancellationToken = default) =>
            Task.FromResult(current = current! with { Status = FocusStatus.Running, ActiveSegmentStartedAtUtc = at, TargetEndAtUtc = targetEndAtUtc });

        public Task<FocusSession> CompleteAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default)
        {
            CompleteCalls++;
            return Task.FromResult(current = current! with { Status = FocusStatus.Completed, DurationSeconds = durationSeconds, EndedAtUtc = at });
        }

        public Task<FocusSession> CancelAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default) =>
            Task.FromResult(current = current! with { Status = FocusStatus.Cancelled, DurationSeconds = durationSeconds, EndedAtUtc = at });

        public Task<FocusSession> InterruptAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default) =>
            throw new NotSupportedException();

        public Task<FocusSession?> GetCurrentAsync(CancellationToken cancellationToken = default) => Task.FromResult(current);
        public Task<IReadOnlyList<FocusSession>> GetRecentAsync(int limit = 5, CancellationToken cancellationToken = default) =>
            Task.FromResult<IReadOnlyList<FocusSession>>([]);
        public Task<FocusDaySummary> GetProductiveSummaryAsync(DateOnly localDate, CancellationToken cancellationToken = default) =>
            Task.FromResult(FocusDaySummary.Empty);
    }
}
