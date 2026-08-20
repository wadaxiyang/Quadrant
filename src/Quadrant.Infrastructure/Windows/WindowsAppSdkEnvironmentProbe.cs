using Microsoft.Windows.ApplicationModel.WindowsAppRuntime;

namespace Quadrant.Infrastructure.Windows;

public sealed record WindowsAppSdkEnvironmentProbeResult(
    bool IsAvailable,
    string? RuntimeVersion,
    string? ErrorType,
    string? ErrorMessage);

/// <summary>
/// Diagnostic-only access check for the auto-initialized Windows App SDK runtime.
/// </summary>
public sealed class WindowsAppSdkEnvironmentProbe
{
    public WindowsAppSdkEnvironmentProbeResult Probe()
    {
        try
        {
            return new WindowsAppSdkEnvironmentProbeResult(
                IsAvailable: true,
                RuntimeVersion: ReleaseInfo.AsString,
                ErrorType: null,
                ErrorMessage: null);
        }
        catch (Exception exception)
        {
            return new WindowsAppSdkEnvironmentProbeResult(
                IsAvailable: false,
                RuntimeVersion: null,
                ErrorType: exception.GetType().FullName,
                ErrorMessage: exception.Message);
        }
    }
}
