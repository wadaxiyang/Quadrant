using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public sealed class TaskCardViewModel
{
    public TaskCardViewModel(TaskItem task)
    {
        Id = task.Id;
        Title = task.Title;
        DueAt = task.DueAt;
        ReminderAt = task.ReminderAt;
    }

    public long Id { get; }

    public string Title { get; }

    public DateTimeOffset? DueAt { get; }

    public DateTimeOffset? ReminderAt { get; }

    public string DueText => DueAt is { } due
        ? $"截止 {due.ToLocalTime():yyyy-MM-dd HH:mm}"
        : "无截止时间";

    public string ReminderText => ReminderAt is { } reminder
        ? $"提醒 {reminder.ToLocalTime():yyyy-MM-dd HH:mm}"
        : string.Empty;
}
