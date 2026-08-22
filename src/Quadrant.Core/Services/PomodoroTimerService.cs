using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class PomodoroTimerService
{
    private readonly IFocusSessionService sessions;
    private readonly IClock clock;
    private readonly IFocusCompletionScheduler scheduler;
    private FocusSession? current;
    private PomodoroSettings? settings;
    private IDisposable? scheduled;
    private int generation;
    private int productiveCount;
    private long? lastFocusTaskId;

    public PomodoroTimerService(IFocusSessionService sessions, IClock clock, IFocusCompletionScheduler scheduler)
    {
        this.sessions = sessions;
        this.clock = clock;
        this.scheduler = scheduler;
    }

    public FocusSession? Current => current;
    public int RemainingSeconds => current?.TargetEndAtUtc is { } end
        ? (int)Math.Max(0, Math.Floor((end - clock.UtcNow).TotalSeconds))
        : 0;
    public PomodoroKind? SuggestedNextKind { get; private set; }
    public event EventHandler<FocusSession>? SessionCompleted;

    public async Task<FocusSession> StartAsync(long? taskId, PomodoroKind kind, PomodoroSettings snapshot, CancellationToken cancellationToken = default)
    {
        if (current is not null) throw new InvalidOperationException("A Pomodoro session is already active.");
        snapshot.Validate();
        settings = snapshot;
        if (kind == PomodoroKind.Focus) lastFocusTaskId = taskId;
        var now = clock.UtcNow;
        current = await sessions.StartAsync(new FocusSessionStartRequest(
            kind == PomodoroKind.Focus ? taskId : null,
            FocusMode.Pomodoro,
            kind,
            now.AddMinutes(GetDurationMinutes(kind, snapshot))), cancellationToken);
        Arm();
        return current;
    }

    public async Task<FocusSession> PauseAsync(CancellationToken cancellationToken = default)
    {
        var session = Require(FocusStatus.Running);
        DisposeTimer();
        var elapsed = session.TargetEndAtUtc is { } end
            ? GetDurationMinutes(session.PomodoroKind!.Value, settings!) * 60 - Math.Max(0, (int)Math.Ceiling((end - clock.UtcNow).TotalSeconds))
            : 0;
        current = await sessions.PauseAsync(session.Id, elapsed, clock.UtcNow, cancellationToken);
        return current;
    }

    public async Task<FocusSession> ResumeAsync(CancellationToken cancellationToken = default)
    {
        var session = Require(FocusStatus.Paused);
        var remaining = Math.Max(0, GetDurationMinutes(session.PomodoroKind!.Value, settings!) * 60 - session.DurationSeconds);
        current = await sessions.ResumeAsync(session.Id, clock.UtcNow, clock.UtcNow.AddSeconds(remaining), cancellationToken);
        Arm();
        return current;
    }

    public Task<FocusSession> StopAsync(CancellationToken cancellationToken = default) =>
        CompleteCurrentAsync(autoStart: false, cancellationToken);

    public async Task<FocusSession> CancelAsync(CancellationToken cancellationToken = default)
    {
        var session = Require(FocusStatus.Running, FocusStatus.Paused);
        var elapsed = GetElapsedSeconds(session);
        DisposeTimer();
        current = null;
        return await sessions.CancelAsync(session.Id, elapsed, clock.UtcNow, cancellationToken);
    }

    private void Arm()
    {
        DisposeTimer();
        var capturedGeneration = ++generation;
        var sessionId = current!.Id;
        scheduled = scheduler.Schedule(current.TargetEndAtUtc!.Value, async () =>
        {
            if (capturedGeneration != generation || current?.Id != sessionId) return;
            try { await CompleteCurrentAsync(autoStart: true); }
            catch (InvalidOperationException) { }
        });
    }

    private async Task<FocusSession> CompleteCurrentAsync(bool autoStart, CancellationToken cancellationToken = default)
    {
        var session = Require(FocusStatus.Running, FocusStatus.Paused);
        var snapshot = settings!;
        var elapsed = GetElapsedSeconds(session);
        DisposeTimer();
        current = null;
        var completed = await sessions.CompleteAsync(session.Id, elapsed, clock.UtcNow, cancellationToken);
        UpdateSuggestion(completed, snapshot);
        SessionCompleted?.Invoke(this, completed);

        if (autoStart && completed.PomodoroKind == PomodoroKind.Focus && snapshot.AutoStartBreak && SuggestedNextKind is { } breakKind)
            await StartAsync(null, breakKind, snapshot, cancellationToken);
        else if (autoStart && completed.PomodoroKind is PomodoroKind.ShortBreak or PomodoroKind.LongBreak && snapshot.AutoStartFocus)
            await StartAsync(lastFocusTaskId, PomodoroKind.Focus, snapshot, cancellationToken);

        return completed;
    }

    private void UpdateSuggestion(FocusSession session, PomodoroSettings snapshot)
    {
        if (session.PomodoroKind == PomodoroKind.Focus)
        {
            productiveCount++;
            SuggestedNextKind = productiveCount % snapshot.LongBreakInterval == 0 ? PomodoroKind.LongBreak : PomodoroKind.ShortBreak;
        }
        else
        {
            SuggestedNextKind = PomodoroKind.Focus;
        }
    }

    private int GetElapsedSeconds(FocusSession session) => session.Status == FocusStatus.Paused
        ? session.DurationSeconds
        : GetDurationMinutes(session.PomodoroKind!.Value, settings!) * 60 - RemainingSeconds;

    private FocusSession Require(params FocusStatus[] allowed) =>
        current is not null && allowed.Contains(current.Status)
            ? current
            : throw new InvalidOperationException("Pomodoro is not in a valid state.");

    private static int GetDurationMinutes(PomodoroKind kind, PomodoroSettings snapshot) => kind switch
    {
        PomodoroKind.Focus => snapshot.FocusMinutes,
        PomodoroKind.ShortBreak => snapshot.ShortBreakMinutes,
        _ => snapshot.LongBreakMinutes
    };

    private void DisposeTimer()
    {
        generation++;
        scheduled?.Dispose();
        scheduled = null;
    }
}
