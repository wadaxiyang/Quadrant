using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface ITaskRepository
{
    Task<IReadOnlyList<TaskItem>> GetActiveAsync(CancellationToken cancellationToken = default);

    Task<IReadOnlyList<TaskItem>> GetInboxAsync(int? limit = null, CancellationToken cancellationToken = default);

    Task<IReadOnlyList<TaskItem>> GetCompletedAsync(CancellationToken cancellationToken = default);

    Task<TaskItem?> GetByIdAsync(long id, CancellationToken cancellationToken = default);

    Task<TaskItem> CreateAsync(TaskDraft draft, DateTimeOffset now, CancellationToken cancellationToken = default);

    Task<TaskItem> UpdateAsync(TaskUpdate update, DateTimeOffset now, CancellationToken cancellationToken = default);

    Task<TaskItem> AssignQuadrantAsync(long id, int quadrantId, DateTimeOffset now, CancellationToken cancellationToken = default);

    Task<TaskItem> MoveToInboxAsync(long id, DateTimeOffset now, CancellationToken cancellationToken = default);

    Task<TaskItem> SetCompletedAsync(long id, bool isCompleted, DateTimeOffset now, CancellationToken cancellationToken = default);

    Task<CompletedTaskMutationResult> CompleteWithSnapshotAsync(long id, DateTimeOffset now, Func<TaskItem, TaskDraft?>? nextDraftFactory = null, CancellationToken cancellationToken = default);

    Task<TaskItem> ReopenWithSnapshotRevertedAsync(long id, DateTimeOffset now, CancellationToken cancellationToken = default);

    Task DeleteAsync(long id, CancellationToken cancellationToken = default);
}
