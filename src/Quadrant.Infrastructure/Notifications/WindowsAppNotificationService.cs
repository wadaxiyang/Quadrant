using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Notifications;

public sealed class WindowsAppNotificationService : IDisposable
{
    private bool isRegistered;

    public event EventHandler<NotificationActivation>? ActivationReceived;

    public void Register()
    {
        if (isRegistered)
        {
            return;
        }

        AppNotificationManager.Default.NotificationInvoked += OnNotificationInvoked;
        AppNotificationManager.Default.Register();
        isRegistered = true;
    }

    public void ShowTaskNotification(TaskItem task)
    {
        ArgumentNullException.ThrowIfNull(task);

        var builder = new AppNotificationBuilder()
            .AddArgument("action", "open")
            .AddArgument("taskId", task.Id.ToString(System.Globalization.CultureInfo.InvariantCulture))
            .AddText(task.Title);

        if (task.DueAt is { } dueAt)
        {
            builder.AddText($"截止 {dueAt.ToLocalTime():yyyy-MM-dd HH:mm}");
        }

        builder
            .AddButton(new AppNotificationButton("完成")
                .AddArgument("action", "complete")
                .AddArgument("taskId", task.Id.ToString(System.Globalization.CultureInfo.InvariantCulture)))
            .AddButton(new AppNotificationButton("打开")
                .AddArgument("action", "open")
                .AddArgument("taskId", task.Id.ToString(System.Globalization.CultureInfo.InvariantCulture)));

        AppNotificationManager.Default.Show(builder.BuildNotification());
    }

    public void Dispose()
    {
        if (!isRegistered)
        {
            return;
        }

        AppNotificationManager.Default.NotificationInvoked -= OnNotificationInvoked;
        AppNotificationManager.Default.Unregister();
        isRegistered = false;
    }

    private void OnNotificationInvoked(
        AppNotificationManager sender,
        AppNotificationActivatedEventArgs args)
    {
        if (NotificationActivationParser.TryParse(args.Argument, out var activation) && activation is not null)
        {
            ActivationReceived?.Invoke(this, activation);
        }
    }
}
