using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface IQuadrantRepository
{
    Task<IReadOnlyList<QuadrantDefinition>> GetAllAsync(CancellationToken cancellationToken = default);

    Task<QuadrantDefinition?> GetByIdAsync(int id, CancellationToken cancellationToken = default);

    Task UpdateAsync(QuadrantDefinition quadrant, CancellationToken cancellationToken = default);
}
