using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface ITodayQueryService
{
    Task<TodaySnapshot> GetSnapshotAsync(CancellationToken cancellationToken = default);
}
