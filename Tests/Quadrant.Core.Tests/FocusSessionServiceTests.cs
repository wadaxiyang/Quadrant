using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class FocusSessionServiceTests
{
    [Fact]
    public async Task Start_snapshots_active_classified_task_and_publishes_only_completion()
    {
        var now = new DateTimeOffset(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8));
        var task = new TaskItem(1, "Focus", 2, null, null, null, false, null, now, now);
        var sessions = new FakeSessions(); var changes = new List<AppChange>(); var hub = new AppChangeHub(); using var subscription = hub.Subscribe(changes.Add);
        var service = new FocusSessionService(sessions, new FakeTasks(task), new FixedClock(now), hub);

        var started = await service.StartAsync(new FocusSessionStartRequest(1, FocusMode.Stopwatch));
        var paused = await service.PauseAsync(started.Id, 30, now.AddSeconds(30));
        var resumed = await service.ResumeAsync(started.Id, now.AddMinutes(1));
        var completed = await service.CompleteAsync(started.Id, 90, now.AddMinutes(2));

        Assert.Equal("Focus", started.TaskTitleSnapshot); Assert.Equal(2, started.QuadrantSnapshot);
        Assert.Equal(FocusStatus.Paused, paused.Status); Assert.Equal(FocusStatus.Running, resumed.Status); Assert.Equal(FocusStatus.Completed, completed.Status);
        Assert.Equal([AppChangeKind.FocusSessionCompleted], changes.Select(change => change.Kind));
    }

    [Fact]
    public async Task Start_rejects_inbox_completed_and_second_active_session()
    {
        var now = DateTimeOffset.UtcNow; var sessions = new FakeSessions(); var service = new FocusSessionService(sessions, new FakeTasks(new TaskItem(1, "Inbox", null, null, null, null, false, null, now, now)), new FixedClock(now), new AppChangeHub());
        await Assert.ThrowsAsync<TaskValidationException>(() => service.StartAsync(new FocusSessionStartRequest(1, FocusMode.Stopwatch)));
        await service.StartAsync(new FocusSessionStartRequest(null, FocusMode.Stopwatch));
        await Assert.ThrowsAsync<InvalidOperationException>(() => service.StartAsync(new FocusSessionStartRequest(null, FocusMode.Stopwatch)));
    }

    [Fact]
    public async Task Break_is_unlinked_and_final_sessions_cannot_transition()
    {
        var now = DateTimeOffset.UtcNow; var sessions = new FakeSessions(); var service = new FocusSessionService(sessions, new FakeTasks(null), new FixedClock(now), new AppChangeHub());
        await Assert.ThrowsAsync<TaskValidationException>(() => service.StartAsync(new FocusSessionStartRequest(1, FocusMode.Pomodoro, PomodoroKind.ShortBreak)));
        var session = await service.StartAsync(new FocusSessionStartRequest(null, FocusMode.Pomodoro, PomodoroKind.ShortBreak));
        Assert.False(FocusSessionRules.IsProductive(session));
        await service.CancelAsync(session.Id, 0, now);
        await Assert.ThrowsAsync<InvalidOperationException>(() => service.ResumeAsync(session.Id, now));
    }

    private sealed class FixedClock(DateTimeOffset now) : IClock { public DateTimeOffset UtcNow=>now.ToUniversalTime(); public DateTimeOffset LocalNow=>now; public DateOnly LocalDate=>DateOnly.FromDateTime(now.Date); public TimeZoneInfo LocalTimeZone=>TimeZoneInfo.Utc; public long GetTimestamp()=>0; public TimeSpan GetElapsedTime(long a,long b)=>TimeSpan.Zero; }
    private sealed class FakeSessions : IFocusSessionRepository
    {
        private readonly Dictionary<string, FocusSession> values=[];
        public Task<FocusSession?> GetCurrentAsync(CancellationToken ct=default)=>Task.FromResult(values.Values.FirstOrDefault(x=>x.Status is FocusStatus.Running or FocusStatus.Paused));
        public Task<FocusSession?> CreateIfNoCurrentAsync(FocusSession s,CancellationToken ct=default){if(values.Values.Any(x=>x.Status is FocusStatus.Running or FocusStatus.Paused))return Task.FromResult<FocusSession?>(null);values[s.Id]=s;return Task.FromResult<FocusSession?>(s);}
        public Task<FocusSession?> GetByIdAsync(string id,CancellationToken ct=default)=>Task.FromResult(values.GetValueOrDefault(id));
        public Task<FocusSession?> TransitionAsync(FocusSession s,FocusStatus expected,CancellationToken ct=default){if(!values.TryGetValue(s.Id,out var old)||old.Status!=expected)return Task.FromResult<FocusSession?>(null);values[s.Id]=s;return Task.FromResult<FocusSession?>(s);}
        public Task<IReadOnlyList<FocusSession>> GetRecentAsync(int limit=5,CancellationToken ct=default)=>Task.FromResult<IReadOnlyList<FocusSession>>(values.Values.OrderByDescending(x=>x.StartedAtUtc).Take(limit).ToArray());
    }
    private sealed class FakeTasks(TaskItem? task) : ITaskRepository
    {
        public Task<TaskItem?> GetByIdAsync(long id,CancellationToken ct=default)=>Task.FromResult(task?.Id==id?task:null); public Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken ct=default)=>Task.FromResult<IReadOnlyList<TaskItem>>([]); public Task<IReadOnlyList<TaskItem>> GetInboxAsync(int? l=null,CancellationToken ct=default)=>Task.FromResult<IReadOnlyList<TaskItem>>([]); public Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken ct=default)=>Task.FromResult<IReadOnlyList<TaskItem>>([]); public Task<TaskItem> CreateAsync(TaskDraft d,DateTimeOffset n,CancellationToken ct=default)=>throw new NotSupportedException(); public Task<TaskItem> UpdateAsync(TaskUpdate u,DateTimeOffset n,CancellationToken ct=default)=>throw new NotSupportedException(); public Task<TaskItem> AssignQuadrantAsync(long i,int q,DateTimeOffset n,CancellationToken ct=default)=>throw new NotSupportedException(); public Task<TaskItem> MoveToInboxAsync(long i,DateTimeOffset n,CancellationToken ct=default)=>throw new NotSupportedException(); public Task<TaskItem> SetCompletedAsync(long i,bool b,DateTimeOffset n,CancellationToken ct=default)=>throw new NotSupportedException(); public Task<CompletedTaskMutationResult> CompleteWithSnapshotAsync(long i,DateTimeOffset n,Func<TaskItem,TaskDraft?>? f=null,CancellationToken ct=default)=>throw new NotSupportedException(); public Task<TaskItem> ReopenWithSnapshotRevertedAsync(long i,DateTimeOffset n,CancellationToken ct=default)=>throw new NotSupportedException(); public Task DeleteAsync(long i,CancellationToken ct=default)=>throw new NotSupportedException();
    }
}
