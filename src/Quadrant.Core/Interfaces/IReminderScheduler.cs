using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface IReminderScheduler
{
    Task ScheduleAsync(TaskItem task, CancellationToken cancellationToken = default);

    Task CancelAsync(long taskId, CancellationToken cancellationToken = default);

    Task RescheduleAsync(TaskItem task, CancellationToken cancellationToken = default);
}
