using Quadrant.Infrastructure.Notifications;
using Xunit;

namespace Quadrant.Infrastructure.Tests;

public sealed class WindowsReminderSchedulerTests
{
    [Theory]
    [InlineData(1, "q1")]
    [InlineData(255, "qFF")]
    [InlineData(123456789, "q75BCD15")]
    public void Uses_stable_short_tag(long taskId, string expected)
    {
        Assert.Equal(expected, WindowsReminderScheduler.GetTag(taskId));
        Assert.True(expected.Length <= 16);
    }
}
