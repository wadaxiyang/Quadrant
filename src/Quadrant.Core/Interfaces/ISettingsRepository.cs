using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface ISettingsRepository
{
    Task<AppSettings> GetAsync(CancellationToken cancellationToken = default);

    Task SaveAsync(
        AppSettings settings,
        IReadOnlyList<QuadrantDefinition> quadrants,
        CancellationToken cancellationToken = default);
}
