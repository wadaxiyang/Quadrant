using Quadrant.Infrastructure.Windows;
using Xunit;

namespace Quadrant.Infrastructure.Tests;

public sealed class GlobalHotkeyServiceTests
{
    [Fact]
    public void IsHotkeyMessage_RecognizesRegisteredMessageId()
    {
        using var service = new GlobalHotkeyService();

        Assert.True(service.IsHotkeyMessage(0x0312, new IntPtr(0x514)));
    }

    [Fact]
    public void IsHotkeyMessage_RejectsOtherMessagesAndIds()
    {
        using var service = new GlobalHotkeyService();

        Assert.False(service.IsHotkeyMessage(0x0311, new IntPtr(0x514)));
        Assert.False(service.IsHotkeyMessage(0x0312, new IntPtr(0x515)));
    }

    [Fact]
    public void RegistrationFailure_ExposesWin32Error()
    {
        var error = new GlobalHotkeyRegistrationFailedEventArgs(5);

        Assert.Equal(5, error.ErrorCode);
        Assert.False(string.IsNullOrWhiteSpace(error.Message));
    }
}
