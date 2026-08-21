using Quadrant.Core.Models;
using Quadrant.Core.Enums;

namespace Quadrant.Core.Interfaces;

public interface IFocusSessionRepository
{
    Task<FocusSession?> GetCurrentAsync(CancellationToken cancellationToken = default);

    Task<FocusSession?> CreateIfNoCurrentAsync(FocusSession session, CancellationToken cancellationToken = default);

    Task<FocusSession?> GetByIdAsync(string id, CancellationToken cancellationToken = default);

    Task<FocusSession?> TransitionAsync(FocusSession session, FocusStatus expectedStatus, CancellationToken cancellationToken = default);

    Task<IReadOnlyList<FocusSession>> GetRecentAsync(int limit = 5, CancellationToken cancellationToken = default);
}
