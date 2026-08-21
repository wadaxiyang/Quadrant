using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface IFocusSessionRepository
{
    Task CreateAsync(FocusSession session, CancellationToken cancellationToken = default);
    Task<FocusSession?> GetByIdAsync(string id, CancellationToken cancellationToken = default);
}
