namespace Quadrant.Core.Interfaces;

public interface IStartupService
{
    bool IsEnabled { get; }

    void SetEnabled(bool enabled, bool startMinimized);
}
