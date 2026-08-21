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

        var task = await service.CreateAsync(new TaskDraft("  Write brief  ", 1, ReminderAt: now.AddHours(1)));

        Assert.Equal("Write brief", repository.LastDraft?.Title);
        Assert.Equal(now, repository.LastNow);
        Assert.Single(scheduler.RescheduledTaskIds);
        Assert.Contains(task.Id, scheduler.RescheduledTaskIds);
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

    [Fact]
    public async Task Create_without_reminder_cancels_any_existing_schedule()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var scheduler = new FakeReminderScheduler();
        var service = new TaskService(new FakeTaskRepository(), scheduler, new FakeClock(now));

        await service.CreateAsync(new TaskDraft("Task", 1));

        Assert.Contains(1, scheduler.CancelledTaskIds);
    }

    [Fact]
    public async Task Snooze_moves_reminder_by_requested_duration_and_reschedules()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var reminder = now.AddMinutes(1);
        var repository = new FakeTaskRepository
        {
            CurrentTask = new TaskItem(1, "Task", 1, null, reminder, null, false, null, now, now)
        };
        var scheduler = new FakeReminderScheduler();
        var service = new TaskService(repository, scheduler, new FakeClock(now));

        var snoozed = await service.SnoozeAsync(1, TimeSpan.FromMinutes(10));

        Assert.Equal(now.AddMinutes(10), snoozed?.ReminderAt);
        Assert.Equal(now.AddMinutes(10), repository.LastUpdate?.ReminderAt);
        Assert.Contains(1, scheduler.RescheduledTaskIds);
    }

    [Fact]
    public async Task Reminder_scheduler_failure_does_not_fail_database_create_or_delete()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var repository = new FakeTaskRepository();
        var scheduler = new FakeReminderScheduler { ThrowOnOperations = true };
        var service = new TaskService(repository, scheduler, new FakeClock(now));

        var created = await service.CreateAsync(new TaskDraft("Task", 1));
        await service.DeleteAsync(created.Id);

        Assert.Equal("Task", repository.LastDraft?.Title);
        Assert.True(scheduler.OperationCount >= 2);
    }

    [Fact]
    public async Task Restore_cancels_old_schedule_instead_of_rescheduling_it()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var scheduler = new FakeReminderScheduler();
        var service = new TaskService(new FakeTaskRepository(), scheduler, new FakeClock(now));

        await service.SetCompletedAsync(1, false);

        Assert.Empty(scheduler.RescheduledTaskIds);
        Assert.Contains(1, scheduler.CancelledTaskIds);
    }

    [Fact]
    public async Task Move_updates_only_the_target_quadrant()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var repository = new FakeTaskRepository
        {
            CurrentTask = new TaskItem(1, "Task", 1, null, null, "note", false, null, now, now)
        };
        var service = new TaskService(repository, new FakeReminderScheduler(), new FakeClock(now));

        var moved = await service.MoveTaskAsync(1, 2);

        Assert.Equal(2, moved?.QuadrantId);
        Assert.Equal(2, repository.LastUpdate?.QuadrantId);
        Assert.Equal("Task", repository.LastUpdate?.Title);
    }

    [Fact]
    public async Task Move_preserves_an_expired_reminder_without_revalidating_or_rescheduling_it()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var repository = new FakeTaskRepository
        {
            CurrentTask = new TaskItem(1, "Task", 1, null, now.AddMinutes(-5), null, false, null, now.AddDays(-1), now.AddDays(-1))
        };
        var scheduler = new FakeReminderScheduler();
        var service = new TaskService(repository, scheduler, new FakeClock(now));

        var moved = await service.MoveTaskAsync(1, 2);

        Assert.Equal(2, moved?.QuadrantId);
        Assert.Equal(now.AddMinutes(-5), moved?.ReminderAt);
        Assert.Empty(scheduler.RescheduledTaskIds);
    }

    [Fact]
    public async Task Update_allows_an_unchanged_expired_reminder_but_rejects_a_new_one()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var expired = now.AddMinutes(-5);
        var repository = new FakeTaskRepository
        {
            CurrentTask = new TaskItem(1, "Task", 1, null, expired, null, false, null, now.AddDays(-1), now.AddDays(-1))
        };
        var service = new TaskService(repository, new FakeReminderScheduler(), new FakeClock(now));

        var updated = await service.UpdateAsync(new TaskUpdate(1, "Renamed", 1, null, expired, null));
        Assert.Equal("Renamed", updated.Title);

        await Assert.ThrowsAsync<TaskValidationException>(() =>
            service.UpdateAsync(new TaskUpdate(1, "Renamed", 1, null, now.AddMinutes(-1), null)));
    }

    [Fact]
    public async Task Move_to_same_quadrant_is_a_no_op()
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var repository = new FakeTaskRepository
        {
            CurrentTask = new TaskItem(1, "Task", 2, null, null, null, false, null, now, now)
        };
        var service = new TaskService(repository, new FakeReminderScheduler(), new FakeClock(now));

        var moved = await service.MoveTaskAsync(1, 2);

        Assert.Same(repository.CurrentTask, moved);
        Assert.Null(repository.LastUpdate);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(5)]
    public async Task Move_to_invalid_quadrant_is_rejected(int targetQuadrantId)
    {
        var now = new DateTimeOffset(2026, 8, 20, 9, 30, 0, TimeSpan.FromHours(8));
        var service = new TaskService(new FakeTaskRepository(), new FakeReminderScheduler(), new FakeClock(now));

        await Assert.ThrowsAsync<TaskValidationException>(() => service.MoveTaskAsync(1, targetQuadrantId));
    }

    private sealed class FakeClock(DateTimeOffset now) : IClock
    {
        public DateTimeOffset Now { get; } = now;
    }

    private sealed class FakeTaskRepository : ITaskRepository
    {
        public TaskDraft? LastDraft { get; private set; }

        public TaskUpdate? LastUpdate { get; private set; }

        public TaskItem? CurrentTask { get; init; }

        public DateTimeOffset LastNow { get; private set; }

        public Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default) =>
            Task.FromResult<IReadOnlyList<TaskItem>>([]);

        public Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default) =>
            Task.FromResult<IReadOnlyList<TaskItem>>([]);

        public Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default) =>
            Task.FromResult(CurrentTask);

        public Task<TaskItem> CreateAsync(TaskDraft draft, DateTimeOffset now, CancellationToken cancellationToken = default)
        {
            LastDraft = draft;
            LastNow = now;
            return Task.FromResult(new TaskItem(1, draft.Title, draft.QuadrantId, draft.DueAt, draft.ReminderAt, draft.Note, false, null, now, now));
        }

        public Task<TaskItem> UpdateAsync(TaskUpdate update, DateTimeOffset now, CancellationToken cancellationToken = default)
        {
            LastUpdate = update;
            return Task.FromResult(new TaskItem(update.Id, update.Title, update.QuadrantId, update.DueAt, update.ReminderAt, update.Note, false, null, now, now));
        }

        public Task<TaskItem> SetCompletedAsync(long id, bool isCompleted, DateTimeOffset now, CancellationToken cancellationToken = default) =>
            Task.FromResult(new TaskItem(id, "Task", 1, null, null, null, isCompleted, isCompleted ? now : null, now, now));

        public Task<CompletedTaskMutationResult> CompleteWithSnapshotAsync(long id, DateTimeOffset now, CancellationToken cancellationToken = default)
        {
            var task = new TaskItem(id, "Task", 1, null, null, null, true, now, now, now);
            return Task.FromResult(new CompletedTaskMutationResult(task, null, false));
        }

        public Task<TaskItem> ReopenWithSnapshotRevertedAsync(long id, DateTimeOffset now, CancellationToken cancellationToken = default) =>
            Task.FromResult(new TaskItem(id, "Task", 1, null, null, null, false, null, now, now));

        public Task DeleteAsync(long id, CancellationToken cancellationToken = default) =>
            Task.CompletedTask;
    }

    private sealed class FakeReminderScheduler : IReminderScheduler
    {
        public List<long> CancelledTaskIds { get; } = [];

        public List<long> RescheduledTaskIds { get; } = [];

        public bool ThrowOnOperations { get; init; }

        public int OperationCount { get; private set; }

        public Task ScheduleAsync(TaskItem task, CancellationToken cancellationToken = default)
        {
            return Task.CompletedTask;
        }

        public Task CancelAsync(long taskId, CancellationToken cancellationToken = default)
        {
            OperationCount++;
            CancelledTaskIds.Add(taskId);
            if (ThrowOnOperations)
            {
                throw new InvalidOperationException("simulated scheduler failure");
            }

            return Task.CompletedTask;
        }

        public Task RescheduleAsync(TaskItem task, CancellationToken cancellationToken = default)
        {
            OperationCount++;
            RescheduledTaskIds.Add(task.Id);
            if (ThrowOnOperations)
            {
                throw new InvalidOperationException("simulated scheduler failure");
            }

            return Task.CompletedTask;
        }
    }
}
