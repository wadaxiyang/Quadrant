using Quadrant.Core.Models;
using System.Windows.Input;

namespace Quadrant.App.ViewModels;

public sealed class TaskCardViewModel
{
    public TaskCardViewModel(TaskItem task, ICommand editCommand, ICommand completeCommand, ICommand deleteCommand, DateTimeOffset now)
    {
        Id = task.Id;
        QuadrantId = task.QuadrantId;
        Title = task.Title;
        DueAt = task.DueAt;
        ReminderAt = task.ReminderAt;
        EditCommand = editCommand;
        CompleteCommand = completeCommand;
        DeleteCommand = deleteCommand;
        IsOverdue = task.DueAt is { } due && !task.IsCompleted && due < now;
        DueStatusText = IsOverdue ? "已逾期" : task.DueAt is { } dueAt && dueAt.ToLocalTime().Date == now.ToLocalTime().Date ? "今天" : string.Empty;
    }

    public long Id { get; }

    public int QuadrantId { get; }

    public string Title { get; }

    public DateTimeOffset? DueAt { get; }

    public DateTimeOffset? ReminderAt { get; }

    public string DueText => DueAt is { } due
        ? $"截止 {due.ToLocalTime():yyyy-MM-dd HH:mm}"
        : "无截止时间";

    public string ReminderText => ReminderAt is { } reminder
        ? $"提醒 {reminder.ToLocalTime():yyyy-MM-dd HH:mm}"
        : string.Empty;

    public bool IsOverdue { get; }

    public string DueStatusText { get; }

    public ICommand EditCommand { get; }

    public ICommand CompleteCommand { get; }

    public ICommand DeleteCommand { get; }
}
