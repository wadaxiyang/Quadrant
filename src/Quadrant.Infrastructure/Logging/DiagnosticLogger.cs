namespace Quadrant.Infrastructure.Logging;

public sealed class DiagnosticLogger : Quadrant.Core.Interfaces.IDiagnosticLogger
{
    private const long MaxFileBytes = 1024 * 1024;
    private const int RetainedFiles = 3;
    private readonly object sync = new();

    public DiagnosticLogger(string localAppDataPath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(localAppDataPath);
        LogDirectory = Path.Combine(localAppDataPath, "Quadrant", "logs");
    }

    public string LogDirectory { get; }

    public string LogFilePath => Path.Combine(LogDirectory, "quadrant.log");

    public void Warning(string message, Exception? exception = null) => Write("WARN", message, exception);

    public void Error(string message, Exception? exception = null) => Write("ERROR", message, exception);

    private void Write(string level, string message, Exception? exception)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(message);
        var line = $"{DateTimeOffset.Now:O} [{level}] {message}";
        if (exception is not null)
        {
            line += $" | {exception.GetType().Name}: {exception.Message}\n{exception.StackTrace}";
        }

        try
        {
            lock (sync)
            {
                Directory.CreateDirectory(LogDirectory);
                RotateIfNeeded(line.Length + Environment.NewLine.Length);
                File.AppendAllText(LogFilePath, line + Environment.NewLine);
            }
        }
        catch (Exception loggingException)
        {
            System.Diagnostics.Debug.WriteLine($"Diagnostic logging failed: {loggingException}");
        }
    }

    private void RotateIfNeeded(int incomingLength)
    {
        if (!File.Exists(LogFilePath) || new FileInfo(LogFilePath).Length + incomingLength <= MaxFileBytes)
        {
            return;
        }

        var oldest = $"{LogFilePath}.{RetainedFiles}";
        if (File.Exists(oldest))
        {
            File.Delete(oldest);
        }

        for (var index = RetainedFiles - 1; index >= 1; index--)
        {
            var source = $"{LogFilePath}.{index}";
            var destination = $"{LogFilePath}.{index + 1}";
            if (File.Exists(source))
            {
                File.Move(source, destination, true);
            }
        }

        File.Move(LogFilePath, $"{LogFilePath}.1", true);
    }
}
