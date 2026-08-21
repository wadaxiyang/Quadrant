using Quadrant.Infrastructure.Notifications;
using Xunit;

namespace Quadrant.Infrastructure.Tests;

public sealed class NotificationActivationParserTests
{
    [Theory]
    [InlineData("action=complete&taskId=42", "complete", 42)]
    [InlineData("taskId=7&action=open", "open", 7)]
    public void Parses_supported_activation(string argument, string action, long taskId)
    {
        Assert.True(NotificationActivationParser.TryParse(argument, out var activation));
        Assert.NotNull(activation);
        Assert.Equal(action, activation.Action);
        Assert.Equal(taskId, activation.TaskId);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("action=snooze&taskId=42")]
    [InlineData("action=open&taskId=0")]
    [InlineData("action=open&taskId=not-a-number")]
    [InlineData("action=open")]
    public void Rejects_untrusted_or_unsupported_activation(string? argument)
    {
        Assert.False(NotificationActivationParser.TryParse(argument, out _));
    }
}
