namespace Quadrant.Infrastructure.Storage;

public sealed class LocalAppDataPathProvider
{
    public string DatabasePath { get; }

    public LocalAppDataPathProvider()
        : this(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData))
    {
    }

    public LocalAppDataPathProvider(string localAppDataPath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(localAppDataPath);
        DatabasePath = Path.Combine(localAppDataPath, "Quadrant", "quadrant.db");
    }
}
