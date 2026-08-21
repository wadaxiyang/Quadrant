using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface ITodayTaskRepository
{
    Task<IReadOnlyList<TaskItem>> GetTodayCandidatesAsync(DateOnly localToday, CancellationToken cancellationToken = default);
}
