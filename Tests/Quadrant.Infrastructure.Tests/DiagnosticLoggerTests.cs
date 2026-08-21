using Quadrant.Infrastructure.Logging;
using Xunit;

namespace Quadrant.Infrastructure.Tests;

public sealed class DiagnosticLoggerTests
{
    [Fact]
    public void Writes_warning_to_local_app_data_logs()
    {
        var directory = Path.Combine(Path.GetTempPath(), "QuadrantLoggerTests", Guid.NewGuid().ToString("N"));
        try
        {
            var logger = new DiagnosticLogger(directory);
            logger.Warning("test warning");

            var content = File.ReadAllText(logger.LogFilePath);
            Assert.Contains("[WARN] test warning", content, StringComparison.Ordinal);
        }
        finally
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, true);
            }
        }
    }
}
