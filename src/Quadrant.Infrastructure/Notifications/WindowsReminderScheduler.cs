using Microsoft.Windows.AppNotifications.Builder;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Windows.Data.Xml.Dom;
using Windows.UI.Notifications;

namespace Quadrant.Infrastructure.Notifications;

public sealed class WindowsReminderScheduler : IReminderScheduler
{
    private const string Group = "Quadrant";
    private readonly Func<ToastNotifier> notifierFactory;

    public WindowsReminderScheduler()
        : this(ToastNotificationManager.CreateToastNotifier)
    {
    }

    internal WindowsReminderScheduler(Func<ToastNotifier> notifierFactory)
    {
        this.notifierFactory = notifierFactory ?? throw new ArgumentNullException(nameof(notifierFactory));
    }

    public Task ScheduleAsync(TaskItem task, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(task);
        cancellationToken.ThrowIfCancellationRequested();
        if (task.IsCompleted || task.ReminderAt is null || task.ReminderAt <= DateTimeOffset.Now)
        {
            return Task.CompletedTask;
        }

        var scheduled = BuildScheduledNotification(task);
        notifierFactory().AddToSchedule(scheduled);
        return Task.CompletedTask;
    }

    public Task CancelAsync(long taskId, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var notifier = notifierFactory();
        foreach (var scheduled in notifier.GetScheduledToastNotifications())
        {
            if (scheduled.Tag == GetTag(taskId) && scheduled.Group == Group)
            {
                notifier.RemoveFromSchedule(scheduled);
            }
        }

        return Task.CompletedTask;
    }

    public async Task RescheduleAsync(TaskItem task, CancellationToken cancellationToken = default)
    {
        await CancelAsync(task.Id, cancellationToken);
        await ScheduleAsync(task, cancellationToken);
    }

    public async Task<MissedReminderResult> ReconcileAsync(
        IReadOnlyList<TaskItem> activeTasks,
        DateTimeOffset now,
        CancellationToken cancellationToken = default)
    {
        var missed = new List<TaskItem>();
        foreach (var task in activeTasks)
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (task.ReminderAt is not { } reminderAt)
            {
                continue;
            }

            if (reminderAt < now)
            {
                missed.Add(task);
            }
            else
            {
                await RescheduleAsync(task, cancellationToken);
            }
        }

        return new MissedReminderResult(missed);
    }

    public static string GetTag(long taskId) => $"q{taskId:X}";

    private static ScheduledToastNotification BuildScheduledNotification(TaskItem task)
    {
        var taskId = task.Id.ToString(System.Globalization.CultureInfo.InvariantCulture);
        var builder = new AppNotificationBuilder()
            .AddArgument("action", "open")
            .AddArgument("taskId", taskId)
            .AddText(task.Title);

        if (task.DueAt is { } dueAt)
        {
            builder.AddText($"截止 {dueAt.ToLocalTime():yyyy-MM-dd HH:mm}");
        }

        builder
            .AddButton(new AppNotificationButton("完成")
                .AddArgument("action", "complete")
                .AddArgument("taskId", taskId))
            .AddButton(new AppNotificationButton("延后 10 分钟")
                .AddArgument("action", "snooze10")
                .AddArgument("taskId", taskId))
            .AddButton(new AppNotificationButton("打开")
                .AddArgument("action", "open")
                .AddArgument("taskId", taskId));

        var document = new XmlDocument();
        document.LoadXml(builder.BuildNotification().Payload);
        var scheduled = new ScheduledToastNotification(document, task.ReminderAt!.Value);
        scheduled.Tag = GetTag(task.Id);
        scheduled.Group = Group;
        // Windows documents an approximately five-minute delivery window. A missed
        // notification is surfaced in-app during reconciliation rather than replayed.
        return scheduled;
    }
}

public sealed record MissedReminderResult(IReadOnlyList<TaskItem> Tasks);
