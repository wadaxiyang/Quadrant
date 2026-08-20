using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class TaskService : ITaskService
{
    private readonly ITaskRepository repository;
    private readonly IReminderScheduler reminderScheduler;
    private readonly IClock clock;

    public TaskService(
        ITaskRepository repository,
        IReminderScheduler reminderScheduler,
        IClock clock)
    {
        this.repository = repository ?? throw new ArgumentNullException(nameof(repository));
        this.reminderScheduler = reminderScheduler ?? throw new ArgumentNullException(nameof(reminderScheduler));
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
    }

    public Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default) =>
        repository.GetActiveAsync(cancellationToken);

    public Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default) =>
        repository.GetCompletedAsync(cancellationToken);

    public Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default) =>
        repository.GetByIdAsync(id, cancellationToken);

    public async Task<TaskItem> CreateAsync(TaskDraft draft, CancellationToken cancellationToken = default)
    {
        var validatedDraft = TaskRules.Validate(draft);
        var task = await repository.CreateAsync(validatedDraft, clock.Now, cancellationToken);
        await reminderScheduler.ScheduleAsync(task, cancellationToken);
        return task;
    }

    public async Task<TaskItem> UpdateAsync(TaskUpdate update, CancellationToken cancellationToken = default)
    {
        var validatedUpdate = TaskRules.Validate(update);
        var task = await repository.UpdateAsync(validatedUpdate, clock.Now, cancellationToken);
        await reminderScheduler.RescheduleAsync(task, cancellationToken);
        return task;
    }

    public async Task<TaskItem> SetCompletedAsync(
        long id,
        bool isCompleted,
        CancellationToken cancellationToken = default)
    {
        var task = await repository.SetCompletedAsync(id, isCompleted, clock.Now, cancellationToken);
        if (isCompleted)
        {
            await reminderScheduler.CancelAsync(id, cancellationToken);
        }
        else
        {
            await reminderScheduler.RescheduleAsync(task, cancellationToken);
        }

        return task;
    }

    public async Task DeleteAsync(long id, CancellationToken cancellationToken = default)
    {
        await repository.DeleteAsync(id, cancellationToken);
        await reminderScheduler.CancelAsync(id, cancellationToken);
    }
}
