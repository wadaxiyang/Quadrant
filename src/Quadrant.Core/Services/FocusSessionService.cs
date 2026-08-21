using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class FocusSessionService : IFocusSessionService
{
    private readonly IFocusSessionRepository repository;
    private readonly ITaskRepository taskRepository;
    private readonly IClock clock;
    private readonly IAppChangeHub appChangeHub;

    public FocusSessionService(IFocusSessionRepository repository, ITaskRepository taskRepository, IClock clock, IAppChangeHub appChangeHub)
    {
        this.repository = repository ?? throw new ArgumentNullException(nameof(repository));
        this.taskRepository = taskRepository ?? throw new ArgumentNullException(nameof(taskRepository));
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
        this.appChangeHub = appChangeHub ?? throw new ArgumentNullException(nameof(appChangeHub));
    }

    public async Task<FocusSession> StartAsync(FocusSessionStartRequest request, CancellationToken cancellationToken = default)
    {
        FocusSessionRules.ValidateStart(request);
        TaskItem? linkedTask = null;
        if (request.TaskId is { } taskId)
        {
            linkedTask = await taskRepository.GetByIdAsync(taskId, cancellationToken)
                ?? throw new TaskValidationException("Focus task was not found.");
            if (linkedTask.IsCompleted || linkedTask.QuadrantId is null)
            {
                throw new TaskValidationException("Focus task must be active and classified.");
            }
        }
        var now = clock.UtcNow;
        var session = new FocusSession(Guid.NewGuid().ToString("N"), request.TaskId, request.Mode, now, now,
            null, request.TargetEndAtUtc?.ToUniversalTime(), 0, FocusStatus.Running, request.PomodoroKind,
            clock.LocalDate, linkedTask?.Title, linkedTask?.QuadrantId);
        var created = await repository.CreateIfNoCurrentAsync(session, cancellationToken);
        if (created is null)
        {
            throw new InvalidOperationException("A focus session is already running or paused.");
        }

        return created;
    }

    public Task<FocusSession?> GetCurrentAsync(CancellationToken cancellationToken = default) => repository.GetCurrentAsync(cancellationToken);
    public Task<IReadOnlyList<FocusSession>> GetRecentAsync(int limit = 5, CancellationToken cancellationToken = default) => repository.GetRecentAsync(limit, cancellationToken);

    public Task<FocusSession> PauseAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default) =>
        TransitionAsync(id, FocusStatus.Running, FocusStatus.Paused, durationSeconds, at, cancellationToken);
    public Task<FocusSession> ResumeAsync(string id, DateTimeOffset at, CancellationToken cancellationToken = default) =>
        TransitionAsync(id, FocusStatus.Paused, FocusStatus.Running, null, at, cancellationToken);
    public Task<FocusSession> CompleteAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default) =>
        TransitionAsync(id, null, FocusStatus.Completed, durationSeconds, at, cancellationToken);
    public Task<FocusSession> InterruptAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default) =>
        TransitionAsync(id, null, FocusStatus.Interrupted, durationSeconds, at, cancellationToken);
    public Task<FocusSession> CancelAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default) =>
        TransitionAsync(id, null, FocusStatus.Cancelled, durationSeconds, at, cancellationToken);

    private async Task<FocusSession> TransitionAsync(string id, FocusStatus? expectedStatus, FocusStatus targetStatus, int? durationSeconds, DateTimeOffset at, CancellationToken cancellationToken)
    {
        if (string.IsNullOrWhiteSpace(id)) throw new ArgumentException("Session ID is required.", nameof(id));
        if (durationSeconds is { } duration) FocusSessionRules.ValidateDuration(duration);
        var current = await repository.GetByIdAsync(id, cancellationToken) ?? throw new InvalidOperationException($"Focus session {id} was not found.");
        var expected = expectedStatus ?? current.Status;
        if (current.Status != expected || current.Status is FocusStatus.Completed or FocusStatus.Interrupted or FocusStatus.Cancelled)
        {
            throw new InvalidOperationException($"Focus session cannot transition from {current.Status} to {targetStatus}.");
        }

        var instant = at.ToUniversalTime();
        var updated = targetStatus switch
        {
            FocusStatus.Paused => current with { Status = targetStatus, DurationSeconds = durationSeconds!.Value, ActiveSegmentStartedAtUtc = null },
            FocusStatus.Running => current with { Status = targetStatus, ActiveSegmentStartedAtUtc = instant },
            _ => current with { Status = targetStatus, DurationSeconds = durationSeconds!.Value, ActiveSegmentStartedAtUtc = null, EndedAtUtc = instant }
        };
        var result = await repository.TransitionAsync(updated, expected, cancellationToken)
            ?? throw new InvalidOperationException("Focus session changed concurrently.");
        if (result.Status == FocusStatus.Completed)
        {
            appChangeHub.Publish(new AppChange(0, AppChangeKind.FocusSessionCompleted));
        }

        return result;
    }
}
