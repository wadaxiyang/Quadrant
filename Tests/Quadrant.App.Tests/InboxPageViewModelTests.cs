using Quadrant.App.ViewModels;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class InboxPageViewModelTests
{
    [Fact]
    public async Task Load_classify_and_change_events_keep_the_inbox_collection_current()
    {
        var first = InboxTask(1, "First");
        var service = new FakeTaskService([first]);
        var hub = new AppChangeHub();
        var originalContext = SynchronizationContext.Current;
        SynchronizationContext.SetSynchronizationContext(new ImmediateSynchronizationContext());
        try
        {
            using var viewModel = new InboxPageViewModel(service, hub);
            await viewModel.ActivateAsync();
            Assert.Equal(1, viewModel.Count);

            await viewModel.AssignQuadrantAsync(first, 2);
            Assert.Empty(viewModel.Tasks);

            var second = InboxTask(2, "Second");
            service.Tasks.Add(second);
            hub.Publish(new AppChange(second.Id, AppChangeKind.TaskCreated));
            Assert.Equal([second], viewModel.Tasks);

            viewModel.Deactivate();
            var third = InboxTask(3, "Third");
            service.Tasks.Add(third);
            hub.Publish(new AppChange(third.Id, AppChangeKind.TaskCreated));
            Assert.Equal([second], viewModel.Tasks);
        }
        finally
        {
            SynchronizationContext.SetSynchronizationContext(originalContext);
        }
    }

    [Fact]
    public async Task Load_error_can_be_retried()
    {
        var service = new FakeTaskService([InboxTask(1, "Retry")]) { ThrowOnInboxRead = true };
        using var viewModel = new InboxPageViewModel(service, new AppChangeHub());

        await viewModel.ActivateAsync();
        Assert.True(viewModel.HasError);
        Assert.Empty(viewModel.Tasks);

        service.ThrowOnInboxRead = false;
        await viewModel.LoadAsync();
        Assert.False(viewModel.HasError);
        Assert.Equal(1, viewModel.Count);
    }

    [Fact]
    public async Task Classification_can_be_undone_only_while_task_is_in_expected_quadrant()
    {
        var task = InboxTask(1, "Undo me");
        var service = new FakeTaskService([task]);
        using var viewModel = new InboxPageViewModel(service, new AppChangeHub());
        await viewModel.ActivateAsync();

        var moved = await viewModel.AssignQuadrantAsync(task, 2);
        Assert.NotNull(moved);
        Assert.Empty(viewModel.Tasks);

        var restored = await viewModel.RestoreToInboxAsync(task.Id, 2);
        Assert.NotNull(restored);
        Assert.Equal([restored], viewModel.Tasks);

        service.Tasks[0] = restored! with { QuadrantId = 3 };
        Assert.Null(await viewModel.RestoreToInboxAsync(task.Id, 2));
        Assert.Equal(3, service.Tasks[0].QuadrantId);
    }

    private static TaskItem InboxTask(long id, string title) => new(id, title, null, null, null, null, false, null,
        new DateTimeOffset(2026, 8, 21, 9, 0, 0, TimeSpan.Zero).AddMinutes(id), DateTimeOffset.UtcNow);

    private sealed class ImmediateSynchronizationContext : SynchronizationContext
    {
        public override void Post(SendOrPostCallback callback, object? state) => callback(state);
    }

    private sealed class FakeTaskService(List<TaskItem> tasks) : ITaskService
    {
        public List<TaskItem> Tasks { get; } = tasks;
        public bool ThrowOnInboxRead { get; set; }

        public Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<TaskItem>>([]);
        public Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default) => Task.FromResult<IReadOnlyList<TaskItem>>([]);
        public Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default) => Task.FromResult<TaskItem?>(Tasks.SingleOrDefault(task => task.Id == id));
        public Task<TaskItem> CreateAsync(TaskDraft draft, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<TaskItem> UpdateAsync(TaskUpdate update, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<TaskItem?> MoveTaskAsync(long id, int targetQuadrantId, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<TaskItem> SetCompletedAsync(long id, bool isCompleted, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<TaskItem?> SnoozeAsync(long id, TimeSpan duration, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task DeleteAsync(long id, CancellationToken cancellationToken = default) { Tasks.RemoveAll(task => task.Id == id); return Task.CompletedTask; }

        public Task<IReadOnlyList<TaskItem>> GetInboxAsync(int? limit = null, CancellationToken cancellationToken = default)
        {
            if (ThrowOnInboxRead) throw new InvalidOperationException("simulated read failure");
            return Task.FromResult<IReadOnlyList<TaskItem>>(Tasks.Where(task => !task.IsCompleted && task.QuadrantId is null).OrderBy(task => task.CreatedAt).ToArray());
        }

        public Task<TaskItem> AssignQuadrantAsync(long id, int quadrantId, CancellationToken cancellationToken = default)
        {
            var index = Tasks.FindIndex(task => task.Id == id);
            Tasks[index] = Tasks[index] with { QuadrantId = quadrantId };
            return Task.FromResult(Tasks[index]);
        }

        public Task<TaskItem> MoveToInboxAsync(long id, CancellationToken cancellationToken = default)
        {
            var index = Tasks.FindIndex(task => task.Id == id);
            Tasks[index] = Tasks[index] with { QuadrantId = null };
            return Task.FromResult(Tasks[index]);
        }

        public Task<TaskItem> PlanForDateAsync(long id, DateOnly plannedDate, CancellationToken cancellationToken = default)
        {
            var index = Tasks.FindIndex(task => task.Id == id);
            Tasks[index] = Tasks[index] with { PlannedDate = plannedDate };
            return Task.FromResult(Tasks[index]);
        }

        public Task<TaskItem> PlanForTodayAsync(long id, CancellationToken cancellationToken = default) =>
            PlanForDateAsync(id, new DateOnly(2026, 8, 21), cancellationToken);

        public Task<TaskItem> RemovePlanAsync(long id, CancellationToken cancellationToken = default) =>
            PlanForDateAsync(id, default, cancellationToken);

        public Task<TaskItem> SetEstimateAsync(long id, int? estimatedMinutes, CancellationToken cancellationToken = default)
        {
            var index = Tasks.FindIndex(task => task.Id == id);
            Tasks[index] = Tasks[index] with { EstimatedMinutes = estimatedMinutes };
            return Task.FromResult(Tasks[index]);
        }
    }
}
