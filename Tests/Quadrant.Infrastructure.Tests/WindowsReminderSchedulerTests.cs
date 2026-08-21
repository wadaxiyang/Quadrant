using Quadrant.Infrastructure.Notifications;
using Quadrant.Core.Models;
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

    [Fact]
    public async Task Reconcile_enumerates_once_and_differentially_repairs_the_schedule()
    {
        var now = new DateTimeOffset(2026, 8, 21, 9, 0, 0, TimeSpan.FromHours(8));
        var retainedTask = CreateTask(1, now.AddHours(1));
        var missingTask = CreateTask(2, now.AddHours(2));
        var missedTask = CreateTask(3, now.AddMinutes(-1));
        var store = new FakeReminderScheduleStore(
            new ScheduledReminderEntry("q1", WindowsReminderScheduler.ScheduleGroup, retainedTask.ReminderAt!.Value, new object()),
            new ScheduledReminderEntry("q1", WindowsReminderScheduler.ScheduleGroup, retainedTask.ReminderAt.Value, new object()),
            new ScheduledReminderEntry("q2", WindowsReminderScheduler.ScheduleGroup, now.AddHours(3), new object()),
            new ScheduledReminderEntry("q63", WindowsReminderScheduler.ScheduleGroup, now.AddHours(4), new object()),
            new ScheduledReminderEntry("unrelated", "OtherApp", now.AddHours(4), new object()));
        var scheduler = new WindowsReminderScheduler(store);

        var result = await scheduler.ReconcileAsync([retainedTask, missingTask, missedTask], now);

        Assert.Equal(1, store.GetAllCallCount);
        Assert.Equal([missedTask], result.Tasks);
        Assert.Equal([missingTask], store.AddedTasks);
        Assert.Equal(3, store.RemovedEntries.Count);
        Assert.DoesNotContain(store.RemovedEntries, entry => entry.Group == "OtherApp");
    }

    private static TaskItem CreateTask(long id, DateTimeOffset reminderAt) =>
        new(id, $"Task {id}", 1, null, reminderAt, null, false, null, reminderAt, reminderAt);

    private sealed class FakeReminderScheduleStore(params ScheduledReminderEntry[] entries) : IReminderScheduleStore
    {
        public int GetAllCallCount { get; private set; }

        public List<TaskItem> AddedTasks { get; } = [];

        public List<ScheduledReminderEntry> RemovedEntries { get; } = [];

        public IReadOnlyList<ScheduledReminderEntry> GetAll()
        {
            GetAllCallCount++;
            return entries;
        }

        public void Add(TaskItem task) => AddedTasks.Add(task);

        public void Remove(ScheduledReminderEntry scheduled) => RemovedEntries.Add(scheduled);
    }
}
