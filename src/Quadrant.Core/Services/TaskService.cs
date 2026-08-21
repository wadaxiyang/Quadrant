using Quadrant.Core.Interfaces;
using Quadrant.Core.Enums;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class TaskService : ITaskService
{
    private readonly ITaskRepository repository;
    private readonly IReminderScheduler reminderScheduler;
    private readonly IClock clock;
    private readonly IDiagnosticLogger? diagnosticLogger;
    private readonly IAppChangeHub appChangeHub;
    private readonly IRecurrenceService recurrenceService;

    public TaskService(
        ITaskRepository repository,
        IReminderScheduler reminderScheduler,
        IClock clock,
        IDiagnosticLogger? diagnosticLogger = null,
        IAppChangeHub? appChangeHub = null,
        IRecurrenceService? recurrenceService = null)
    {
        this.repository = repository ?? throw new ArgumentNullException(nameof(repository));
        this.reminderScheduler = reminderScheduler ?? throw new ArgumentNullException(nameof(reminderScheduler));
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
        this.diagnosticLogger = diagnosticLogger;
        this.appChangeHub = appChangeHub ?? new AppChangeHub(diagnosticLogger);
        this.recurrenceService = recurrenceService ?? new RecurrenceService();
    }

    public Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default) =>
        repository.GetActiveAsync(cancellationToken);

    public Task<IReadOnlyList<TaskItem>> GetInboxAsync(int? limit = null, CancellationToken cancellationToken = default) =>
        repository.GetInboxAsync(limit, cancellationToken);

    public Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default) =>
        repository.GetCompletedAsync(cancellationToken);

    public Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default) =>
        repository.GetByIdAsync(id, cancellationToken);

    public async Task<TaskItem> CreateAsync(TaskDraft draft, CancellationToken cancellationToken = default)
    {
        var now = clock.LocalNow;
        var validatedDraft = TaskRules.Validate(draft);
        TaskRules.ValidateReminderAt(validatedDraft.ReminderAt, now);
        var task = await repository.CreateAsync(validatedDraft, now, cancellationToken);
        Publish(task.Id, AppChangeKind.TaskCreated);
        await TrySyncReminderAsync(task, cancellationToken);
        return task;
    }

    public async Task<TaskItem> UpdateAsync(TaskUpdate update, CancellationToken cancellationToken = default)
    {
        var now = clock.LocalNow;
        var validatedUpdate = TaskRules.Validate(update);
        if (validatedUpdate.ReminderAt is { } reminderAt && reminderAt <= now)
        {
            var existing = await repository.GetByIdAsync(validatedUpdate.Id, cancellationToken)
                ?? throw new InvalidOperationException($"Task {validatedUpdate.Id} was not found.");
            if (existing.ReminderAt != validatedUpdate.ReminderAt)
            {
                TaskRules.ValidateReminderAt(validatedUpdate.ReminderAt, now);
            }
        }

        var task = await repository.UpdateAsync(validatedUpdate, now, cancellationToken);
        Publish(task.Id, AppChangeKind.TaskUpdated);
        await TrySyncReminderAsync(task, cancellationToken);
        return task;
    }

    public async Task<TaskItem?> MoveTaskAsync(
        long id,
        int targetQuadrantId,
        CancellationToken cancellationToken = default)
    {
        if (targetQuadrantId is < 1 or > 4)
        {
            throw new TaskValidationException("Quadrant must be between 1 and 4.");
        }

        var task = await repository.GetByIdAsync(id, cancellationToken);
        if (task is null || task.IsCompleted || task.QuadrantId == targetQuadrantId)
        {
            return task;
        }

        // Moving a task does not change its reminder. In particular, an already
        // delivered reminder may legitimately be in the past and must not block
        // quadrant movement or cause an unnecessary OS schedule rebuild.
        var moved = await repository.UpdateAsync(
            new TaskUpdate(task.Id, task.Title, targetQuadrantId, task.DueAt, task.ReminderAt, task.Note,
                task.PlannedDate, task.EstimatedMinutes, task.RecurrenceKind, task.RecurrenceInterval,
                task.RecurrenceSeriesId, task.RecurrenceAnchorDay),
            clock.LocalNow,
            cancellationToken);
        Publish(moved.Id, AppChangeKind.TaskClassified);
        return moved;
    }

    public async Task<TaskItem> AssignQuadrantAsync(long id, int quadrantId, CancellationToken cancellationToken = default)
    {
        TaskRules.ValidateQuadrantId(quadrantId);
        var existing = await GetActiveTaskForClassificationAsync(id, cancellationToken);
        if (existing.QuadrantId == quadrantId)
        {
            return existing;
        }

        var task = await repository.AssignQuadrantAsync(id, quadrantId, clock.LocalNow, cancellationToken);
        Publish(task.Id, AppChangeKind.TaskClassified);
        return task;
    }

    public async Task<TaskItem> MoveToInboxAsync(long id, CancellationToken cancellationToken = default)
    {
        var existing = await GetActiveTaskForClassificationAsync(id, cancellationToken);
        if (existing.QuadrantId is null)
        {
            return existing;
        }

        var task = await repository.MoveToInboxAsync(id, clock.LocalNow, cancellationToken);
        Publish(task.Id, AppChangeKind.TaskClassified);
        return task;
    }

    public Task<TaskItem> PlanForTodayAsync(long id, CancellationToken cancellationToken = default) =>
        PlanForDateAsync(id, clock.LocalDate, cancellationToken);

    public async Task<TaskItem> PlanForDateAsync(long id, DateOnly plannedDate, CancellationToken cancellationToken = default)
    {
        var task = await GetActiveTaskForPlanningAsync(id, cancellationToken);
        if (task.PlannedDate == plannedDate)
        {
            return task;
        }

        return await UpdatePlanningAsync(task, plannedDate, task.EstimatedMinutes, cancellationToken);
    }

    public async Task<TaskItem> RemovePlanAsync(long id, CancellationToken cancellationToken = default)
    {
        var task = await GetActiveTaskForPlanningAsync(id, cancellationToken);
        if (task.PlannedDate is null)
        {
            return task;
        }

        return await UpdatePlanningAsync(task, null, task.EstimatedMinutes, cancellationToken);
    }

    public async Task<TaskItem> SetEstimateAsync(long id, int? estimatedMinutes, CancellationToken cancellationToken = default)
    {
        if (estimatedMinutes is < 1 or > 1440)
        {
            throw new TaskValidationException("Estimated minutes must be between 1 and 1440.");
        }

        var task = await GetActiveTaskForPlanningAsync(id, cancellationToken);
        if (task.EstimatedMinutes == estimatedMinutes)
        {
            return task;
        }

        return await UpdatePlanningAsync(task, task.PlannedDate, estimatedMinutes, cancellationToken);
    }

    public async Task<TaskItem> SetCompletedAsync(
        long id,
        bool isCompleted,
        CancellationToken cancellationToken = default)
    {
        TaskItem task;
        if (isCompleted)
        {
            var now = clock.LocalNow;
            var result = await repository.CompleteWithSnapshotAsync(
                id,
                now,
                source => recurrenceService.BuildNextDraft(source, now, clock.LocalTimeZone),
                cancellationToken);
            task = result.Task;
            if (!result.WasAlreadyCompleted)
            {
                Publish(task.Id, AppChangeKind.TaskCompleted);
                if (result.NextTask is { } nextTask)
                {
                    Publish(nextTask.Id, AppChangeKind.TaskCreated);
                }
            }
            await TryCancelReminderAsync(id, cancellationToken);
            if (result.NextTask is { } nextOccurrence)
            {
                if (nextOccurrence.ReminderAt > now)
                {
                    await TrySyncReminderAsync(nextOccurrence, cancellationToken);
                }
                else
                {
                    await TryCancelReminderAsync(nextOccurrence.Id, cancellationToken);
                }
            }
        }
        else
        {
            task = await repository.ReopenWithSnapshotRevertedAsync(id, clock.LocalNow, cancellationToken);
            Publish(task.Id, AppChangeKind.TaskReopened);
        }
        // Restoring a task never revives an old OS schedule. The DB value is
        // retained for in-app context and can be explicitly rescheduled later.
        if (!isCompleted)
        {
            await TryCancelReminderAsync(id, cancellationToken);
        }

        return task;
    }

    public async Task<TaskItem?> SnoozeAsync(
        long id,
        TimeSpan duration,
        CancellationToken cancellationToken = default)
    {
        if (duration <= TimeSpan.Zero)
        {
            throw new TaskValidationException("Snooze duration must be positive.");
        }

        var task = await repository.GetByIdAsync(id, cancellationToken);
        if (task is null || task.IsCompleted)
        {
            return task;
        }

        var now = clock.LocalNow;
        var reminderAt = now.Add(duration);
        var updated = await repository.UpdateAsync(
            new TaskUpdate(task.Id, task.Title, task.QuadrantId, task.DueAt, reminderAt, task.Note),
            now,
            cancellationToken);
        Publish(updated.Id, AppChangeKind.TaskUpdated);
        await TryRescheduleReminderAsync(updated, cancellationToken);
        return updated;
    }

    public async Task DeleteAsync(long id, CancellationToken cancellationToken = default)
    {
        if (await repository.GetByIdAsync(id, cancellationToken) is null)
        {
            return;
        }

        await repository.DeleteAsync(id, cancellationToken);
        Publish(id, AppChangeKind.TaskDeleted);
        await TryCancelReminderAsync(id, cancellationToken);
    }

    private async Task<TaskItem> GetActiveTaskForClassificationAsync(long id, CancellationToken cancellationToken)
    {
        var task = await repository.GetByIdAsync(id, cancellationToken)
            ?? throw new InvalidOperationException($"Task {id} was not found.");
        if (task.IsCompleted)
        {
            throw new TaskValidationException("Completed tasks cannot be classified.");
        }

        return task;
    }

    private async Task<TaskItem> GetActiveTaskForPlanningAsync(long id, CancellationToken cancellationToken)
    {
        var task = await repository.GetByIdAsync(id, cancellationToken)
            ?? throw new InvalidOperationException($"Task {id} was not found.");
        if (task.IsCompleted)
        {
            throw new TaskValidationException("Completed tasks cannot be planned.");
        }

        return task;
    }

    private async Task<TaskItem> UpdatePlanningAsync(
        TaskItem task,
        DateOnly? plannedDate,
        int? estimatedMinutes,
        CancellationToken cancellationToken)
    {
        var updated = await repository.UpdateAsync(
            new TaskUpdate(task.Id, task.Title, task.QuadrantId, task.DueAt, task.ReminderAt, task.Note,
                plannedDate, estimatedMinutes, task.RecurrenceKind, task.RecurrenceInterval,
                task.RecurrenceSeriesId, task.RecurrenceAnchorDay),
            clock.LocalNow,
            cancellationToken);
        Publish(updated.Id, AppChangeKind.TaskPlanned);
        return updated;
    }

    private void Publish(long taskId, AppChangeKind kind) => appChangeHub.Publish(new AppChange(taskId, kind));

    private async Task TrySyncReminderAsync(TaskItem task, CancellationToken cancellationToken)
    {
        try
        {
            if (task.ReminderAt is null)
            {
                await reminderScheduler.CancelAsync(task.Id, cancellationToken);
                return;
            }

            await reminderScheduler.RescheduleAsync(task, cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            diagnosticLogger?.Warning($"Reminder synchronization failed for task {task.Id}.", exception);
        }
    }

    private async Task TryRescheduleReminderAsync(TaskItem task, CancellationToken cancellationToken)
    {
        try
        {
            await reminderScheduler.RescheduleAsync(task, cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            diagnosticLogger?.Warning($"Reminder rescheduling failed for task {task.Id}.", exception);
        }
    }

    private async Task TryCancelReminderAsync(long taskId, CancellationToken cancellationToken)
    {
        try
        {
            await reminderScheduler.CancelAsync(taskId, cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            diagnosticLogger?.Warning($"Reminder cancellation failed for task {taskId}.", exception);
        }
    }
}
