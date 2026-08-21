using Quadrant.Core.Enums;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class AppChangeHubTests
{
    [Fact]
    public void Disposed_subscription_is_not_called()
    {
        var hub = new AppChangeHub();
        var received = new List<AppChange>();
        var subscription = hub.Subscribe(received.Add);

        hub.Publish(new AppChange(1, AppChangeKind.TaskCreated));
        subscription.Dispose();
        hub.Publish(new AppChange(2, AppChangeKind.TaskUpdated));

        Assert.Equal([new AppChange(1, AppChangeKind.TaskCreated)], received);
    }

    [Fact]
    public void Throwing_subscriber_does_not_prevent_other_subscribers()
    {
        var hub = new AppChangeHub();
        var received = new List<AppChange>();
        using var ignored = hub.Subscribe(_ => throw new InvalidOperationException("simulated subscriber failure"));
        using var active = hub.Subscribe(received.Add);

        hub.Publish(new AppChange(1, AppChangeKind.TaskClassified));

        Assert.Equal([new AppChange(1, AppChangeKind.TaskClassified)], received);
    }
}
