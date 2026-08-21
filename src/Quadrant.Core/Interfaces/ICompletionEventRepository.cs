using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface ICompletionEventRepository
{
    Task CreateAsync(CompletionEvent completionEvent, CancellationToken cancellationToken = default);
    Task<CompletionEvent?> GetByIdAsync(string id, CancellationToken cancellationToken = default);
}
