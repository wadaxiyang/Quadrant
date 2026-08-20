using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class TaskServiceTests
{
    [Fact]
    public async Task Create_uses_injected_clock_and_schedules_task()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var repository = new FakeTaskRepository();
        var scheduler = new FakeReminderScheduler();
        var service = new TaskService(repository, scheduler, new FakeClock(now));

        var task = await service.CreateAsync(new TaskDraft("  Write brief  ", 1));

        Assert.Equal("Write brief", repository.LastDraft?.Title);
        Assert.Equal(now, repository.LastNow);
        Assert.Same(task, scheduler.LastScheduled);
    }

    [Fact]
    public async Task Complete_cancels_reminder_and_delete_cancels_again()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var repository = new FakeTaskRepository();
        var scheduler = new FakeReminderScheduler();
        var service = new TaskService(repository, scheduler, new FakeClock(now));

        await service.SetCompletedAsync(1, true);
        await service.DeleteAsync(1);

        Assert.Equal(2, scheduler.CancelledTaskIds.Count);
        Assert.All(scheduler.CancelledTaskIds, id => Assert.Equal(1, id));
    }

    private sealed class FakeClock(DateTimeOffset now) : IClock
    {
        public DateTimeOffset Now { get; } = now;
    }

    private sealed class FakeTaskRepository : ITaskRepository
    {
        public TaskDraft? LastDraft { get; private set; }

        public DateTimeOffset LastNow { get; private set; }

        public Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default) =>
            Task.FromResult<IReadOnlyList<TaskItem>>([]);

        public Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default) =>
            Task.FromResult<IReadOnlyList<TaskItem>>([]);

        public Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default) =>
            Task.FromResult<TaskItem?>(null);

        public Task<TaskItem> CreateAsync(TaskDraft draft, DateTimeOffset now, CancellationToken cancellationToken = default)
        {
            LastDraft = draft;
            LastNow = now;
            return Task.FromResult(new TaskItem(1, draft.Title, draft.QuadrantId, draft.DueAt, draft.ReminderAt, draft.Note, false, null, now, now));
        }

        public Task<TaskItem> UpdateAsync(TaskUpdate update, DateTimeOffset now, CancellationToken cancellationToken = default) =>
            throw new NotSupportedException();

        public Task<TaskItem> SetCompletedAsync(long id, bool isCompleted, DateTimeOffset now, CancellationToken cancellationToken = default) =>
            Task.FromResult(new TaskItem(id, "Task", 1, null, null, null, isCompleted, isCompleted ? now : null, now, now));

        public Task DeleteAsync(long id, CancellationToken cancellationToken = default) =>
            Task.CompletedTask;
    }

    private sealed class FakeReminderScheduler : IReminderScheduler
    {
        public TaskItem? LastScheduled { get; private set; }

        public List<long> CancelledTaskIds { get; } = [];

        public Task ScheduleAsync(TaskItem task, CancellationToken cancellationToken = default)
        {
            LastScheduled = task;
            return Task.CompletedTask;
        }

        public Task CancelAsync(long taskId, CancellationToken cancellationToken = default)
        {
            CancelledTaskIds.Add(taskId);
            return Task.CompletedTask;
        }

        public Task RescheduleAsync(TaskItem task, CancellationToken cancellationToken = default) =>
            Task.CompletedTask;
    }
}
