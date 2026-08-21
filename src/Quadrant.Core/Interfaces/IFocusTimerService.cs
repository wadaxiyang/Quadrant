using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface IFocusTimerService
{
    FocusTimerSnapshot? Current { get; }
    Task<FocusTimerSnapshot> StartAsync(FocusSessionStartRequest request, CancellationToken cancellationToken = default);
    Task<FocusTimerSnapshot?> RestoreAsync(CancellationToken cancellationToken = default);
    FocusTimerSnapshot? GetSnapshot();
    Task<FocusTimerSnapshot> PauseCurrentAsync(CancellationToken cancellationToken = default);
    Task<FocusTimerSnapshot> ResumeCurrentAsync(CancellationToken cancellationToken = default);
    Task<FocusSession> StopCurrentAsync(CancellationToken cancellationToken = default);
    Task<FocusSession> CancelCurrentAsync(CancellationToken cancellationToken = default);
}
