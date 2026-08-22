namespace Quadrant.Core.Interfaces;

public interface IDataMaintenanceService
{
    Task BackupAsync(string destinationPath, CancellationToken cancellationToken = default);

    Task ExportJsonAsync(string destinationPath, CancellationToken cancellationToken = default);

    Task ClearFocusHistoryAsync(CancellationToken cancellationToken = default);

    Task ClearCompletionHistoryAsync(CancellationToken cancellationToken = default);

    Task ResetAllAsync(CancellationToken cancellationToken = default);
}
