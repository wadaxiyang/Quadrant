using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;
using Quadrant.Core.Models;
using Quadrant.Core.Enums;

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

    public void ShowFocusCompleted(FocusSession session, PomodoroKind? suggestedNextKind)
    {
        ArgumentNullException.ThrowIfNull(session);
        var isBreak = session.PomodoroKind is PomodoroKind.ShortBreak or PomodoroKind.LongBreak;
        var title = isBreak ? "休息结束" : "专注结束";
        var detail = isBreak ? "准备开始下一次专注。" : suggestedNextKind is PomodoroKind.LongBreak ? "本轮完成，建议开始长休息。" : "做得好，建议开始短休息。";
        var builder = new AppNotificationBuilder().AddArgument("action", "focusopen").AddArgument("sessionId", session.Id).AddText(title).AddText(detail)
            .AddButton(new AppNotificationButton("打开 Focus").AddArgument("action", "focusopen").AddArgument("sessionId", session.Id));
        if (!isBreak && suggestedNextKind is not null)
        {
            builder.AddButton(new AppNotificationButton("开始休息").AddArgument("action", "startbreak").AddArgument("sessionId", session.Id));
        }
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
