using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Notifications;

public sealed class NoOpReminderScheduler : IReminderScheduler
{
    public Task ScheduleAsync(TaskItem task, CancellationToken cancellationToken = default) =>
        Task.CompletedTask;

    public Task CancelAsync(long taskId, CancellationToken cancellationToken = default) =>
        Task.CompletedTask;

    public Task RescheduleAsync(TaskItem task, CancellationToken cancellationToken = default) =>
        Task.CompletedTask;
}
