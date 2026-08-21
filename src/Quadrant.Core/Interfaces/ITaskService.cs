using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface ITaskService
{
    Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default);

    Task<IReadOnlyList<TaskItem>> GetInboxAsync(int? limit = null, CancellationToken cancellationToken = default);

    Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default);

    Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default);

    Task<TaskItem> CreateAsync(TaskDraft draft, CancellationToken cancellationToken = default);

    Task<TaskItem> UpdateAsync(TaskUpdate update, CancellationToken cancellationToken = default);

    Task<TaskItem?> MoveTaskAsync(long id, int targetQuadrantId, CancellationToken cancellationToken = default);

    Task<TaskItem> AssignQuadrantAsync(long id, int quadrantId, CancellationToken cancellationToken = default);

    Task<TaskItem> MoveToInboxAsync(long id, CancellationToken cancellationToken = default);

    Task<TaskItem> SetCompletedAsync(long id, bool isCompleted, CancellationToken cancellationToken = default);

    Task<TaskItem?> SnoozeAsync(long id, TimeSpan duration, CancellationToken cancellationToken = default);

    Task DeleteAsync(long id, CancellationToken cancellationToken = default);
}
