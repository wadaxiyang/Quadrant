using Microsoft.Win32;
using Quadrant.Core.Interfaces;

namespace Quadrant.Infrastructure.Windows;

public sealed class RegistryStartupService : IStartupService
{
    private const string RunPath = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    private const string ValueName = "Quadrant";
    private readonly string executablePath;

    public RegistryStartupService(string? executablePath = null) => this.executablePath = executablePath ?? Environment.ProcessPath ?? throw new InvalidOperationException("无法确定应用程序路径。");

    public bool IsEnabled
    {
        get
        {
            using var key = Registry.CurrentUser.OpenSubKey(RunPath, false);
            return key?.GetValue(ValueName) is string;
        }
    }

    public void SetEnabled(bool enabled, bool startMinimized)
    {
        using var key = Registry.CurrentUser.CreateSubKey(RunPath);
        if (!enabled)
        {
            key?.DeleteValue(ValueName, false);
            return;
        }

        var argument = startMinimized ? " --background" : string.Empty;
        key?.SetValue(ValueName, $"\"{executablePath}\"{argument}");
    }
}
