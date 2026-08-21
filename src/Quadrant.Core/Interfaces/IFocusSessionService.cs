using Quadrant.Core.Enums;
using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface IFocusSessionService
{
    Task<FocusSession> StartAsync(FocusSessionStartRequest request, CancellationToken cancellationToken = default);
    Task<FocusSession> PauseAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default);
    Task<FocusSession> ResumeAsync(string id, DateTimeOffset at, CancellationToken cancellationToken = default);
    Task<FocusSession> CompleteAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default);
    Task<FocusSession> InterruptAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default);
    Task<FocusSession> CancelAsync(string id, int durationSeconds, DateTimeOffset at, CancellationToken cancellationToken = default);
    Task<FocusSession?> GetCurrentAsync(CancellationToken cancellationToken = default);
    Task<IReadOnlyList<FocusSession>> GetRecentAsync(int limit = 5, CancellationToken cancellationToken = default);
}
