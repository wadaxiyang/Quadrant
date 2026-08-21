using Microsoft.Windows.AppNotifications.Builder;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Windows.Data.Xml.Dom;
using Windows.UI.Notifications;

namespace Quadrant.Infrastructure.Notifications;

public sealed class WindowsReminderScheduler : IReminderScheduler
{
    internal const string ScheduleGroup = "Quadrant";
    private readonly IReminderScheduleStore store;

    public WindowsReminderScheduler()
        : this(new WindowsReminderScheduleStore())
    {
    }

    internal WindowsReminderScheduler(IReminderScheduleStore store)
    {
        this.store = store ?? throw new ArgumentNullException(nameof(store));
    }

    public Task ScheduleAsync(TaskItem task, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(task);
        cancellationToken.ThrowIfCancellationRequested();
        if (!ShouldSchedule(task, DateTimeOffset.Now))
        {
            return Task.CompletedTask;
        }

        store.Add(task);
        return Task.CompletedTask;
    }

    public Task CancelAsync(long taskId, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var tag = GetTag(taskId);
        foreach (var scheduled in store.GetAll())
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (scheduled.Tag == tag && scheduled.Group == ScheduleGroup)
            {
                store.Remove(scheduled);
            }
        }

        return Task.CompletedTask;
    }

    public async Task RescheduleAsync(TaskItem task, CancellationToken cancellationToken = default)
    {
        await CancelAsync(task.Id, cancellationToken);
        await ScheduleAsync(task, cancellationToken);
    }

    public Task<MissedReminderResult> ReconcileAsync(
        IReadOnlyList<TaskItem> activeTasks,
        DateTimeOffset now,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(activeTasks);
        var missed = new List<TaskItem>();
        var desired = new Dictionary<string, TaskItem>(StringComparer.Ordinal);
        foreach (var task in activeTasks)
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (task.ReminderAt is not { } reminderAt)
            {
                continue;
            }

            if (reminderAt <= now)
            {
                missed.Add(task);
            }
            else if (!task.IsCompleted)
            {
                desired[GetTag(task.Id)] = task;
            }
        }

        // Get the Windows schedule once. Tag + Group are the documented composite
        // key. This removes orphan/duplicate/stale entries in O(tasks + schedules)
        // instead of enumerating the full OS schedule once per task.
        var retained = new HashSet<string>(StringComparer.Ordinal);
        foreach (var scheduled in store.GetAll())
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (scheduled.Group != ScheduleGroup)
            {
                continue;
            }

            if (!desired.TryGetValue(scheduled.Tag, out var task) ||
                retained.Contains(scheduled.Tag) ||
                scheduled.DeliveryTime != task.ReminderAt)
            {
                store.Remove(scheduled);
                continue;
            }

            retained.Add(scheduled.Tag);
        }

        foreach (var (tag, task) in desired)
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (!retained.Contains(tag))
            {
                store.Add(task);
            }
        }

        return Task.FromResult(new MissedReminderResult(missed));
    }

    public static string GetTag(long taskId) => $"q{taskId:X}";

    private static bool ShouldSchedule(TaskItem task, DateTimeOffset now) =>
        !task.IsCompleted && task.ReminderAt is { } reminderAt && reminderAt > now;

    internal static ScheduledToastNotification BuildScheduledNotification(TaskItem task)
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
        var scheduled = new ScheduledToastNotification(document, task.ReminderAt!.Value)
        {
            Tag = GetTag(task.Id),
            Group = ScheduleGroup
        };
        // Windows documents an approximately five-minute delivery window. A missed
        // notification is surfaced in-app during reconciliation rather than replayed.
        return scheduled;
    }
}

internal sealed record ScheduledReminderEntry(
    string Tag,
    string Group,
    DateTimeOffset DeliveryTime,
    object NativeToken);

internal interface IReminderScheduleStore
{
    IReadOnlyList<ScheduledReminderEntry> GetAll();

    void Add(TaskItem task);

    void Remove(ScheduledReminderEntry scheduled);
}

internal sealed class WindowsReminderScheduleStore : IReminderScheduleStore
{
    // The scheduler is composed before AppNotificationManager.Register runs.
    // Creating ToastNotifier here would make App construction fail, so defer it
    // until the first actual scheduling operation after notification setup.
    private readonly Lazy<ToastNotifier> notifier = new(ToastNotificationManager.CreateToastNotifier);

    public IReadOnlyList<ScheduledReminderEntry> GetAll() =>
        notifier.Value.GetScheduledToastNotifications()
            .Select(item => new ScheduledReminderEntry(item.Tag, item.Group, item.DeliveryTime, item))
            .ToArray();

    public void Add(TaskItem task) =>
        notifier.Value.AddToSchedule(WindowsReminderScheduler.BuildScheduledNotification(task));

    public void Remove(ScheduledReminderEntry scheduled) =>
        notifier.Value.RemoveFromSchedule((ScheduledToastNotification)scheduled.NativeToken);
}

public sealed record MissedReminderResult(IReadOnlyList<TaskItem> Tasks);
