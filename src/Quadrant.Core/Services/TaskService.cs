using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class TaskService : ITaskService
{
    private readonly ITaskRepository repository;
    private readonly IReminderScheduler reminderScheduler;
    private readonly IClock clock;
    private readonly IDiagnosticLogger? diagnosticLogger;

    public TaskService(
        ITaskRepository repository,
        IReminderScheduler reminderScheduler,
        IClock clock,
        IDiagnosticLogger? diagnosticLogger = null)
    {
        this.repository = repository ?? throw new ArgumentNullException(nameof(repository));
        this.reminderScheduler = reminderScheduler ?? throw new ArgumentNullException(nameof(reminderScheduler));
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
        this.diagnosticLogger = diagnosticLogger;
    }

    public Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default) =>
        repository.GetActiveAsync(cancellationToken);

    public Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default) =>
        repository.GetCompletedAsync(cancellationToken);

    public Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default) =>
        repository.GetByIdAsync(id, cancellationToken);

    public async Task<TaskItem> CreateAsync(TaskDraft draft, CancellationToken cancellationToken = default)
    {
        var now = clock.Now;
        var validatedDraft = TaskRules.Validate(draft);
        TaskRules.ValidateReminderAt(validatedDraft.ReminderAt, now);
        var task = await repository.CreateAsync(validatedDraft, now, cancellationToken);
        await TrySyncReminderAsync(task, cancellationToken);
        return task;
    }

    public async Task<TaskItem> UpdateAsync(TaskUpdate update, CancellationToken cancellationToken = default)
    {
        var now = clock.Now;
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
        return await repository.UpdateAsync(
            new TaskUpdate(task.Id, task.Title, targetQuadrantId, task.DueAt, task.ReminderAt, task.Note),
            clock.Now,
            cancellationToken);
    }

    public async Task<TaskItem> SetCompletedAsync(
        long id,
        bool isCompleted,
        CancellationToken cancellationToken = default)
    {
        var task = isCompleted
            ? (await repository.CompleteWithSnapshotAsync(id, clock.Now, cancellationToken)).Task
            : await repository.ReopenWithSnapshotRevertedAsync(id, clock.Now, cancellationToken);
        // Restoring a task never revives an old OS schedule. The DB value is
        // retained for in-app context and can be explicitly rescheduled later.
        await TryCancelReminderAsync(id, cancellationToken);

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

        var reminderAt = clock.Now.Add(duration);
        var updated = await repository.UpdateAsync(
            new TaskUpdate(task.Id, task.Title, task.QuadrantId, task.DueAt, reminderAt, task.Note),
            clock.Now,
            cancellationToken);
        await TryRescheduleReminderAsync(updated, cancellationToken);
        return updated;
    }

    public async Task DeleteAsync(long id, CancellationToken cancellationToken = default)
    {
        await repository.DeleteAsync(id, cancellationToken);
        await TryCancelReminderAsync(id, cancellationToken);
    }

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
