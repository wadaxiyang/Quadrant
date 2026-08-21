namespace Quadrant.Core.Interfaces;

public interface IDiagnosticLogger
{
    void Warning(string message, Exception? exception = null);
}
