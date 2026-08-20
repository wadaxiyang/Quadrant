using Quadrant.Core.Models;
using System.Windows.Input;

namespace Quadrant.App.ViewModels;

public sealed class TaskCardViewModel
{
    public TaskCardViewModel(TaskItem task, ICommand editCommand, ICommand completeCommand, ICommand deleteCommand)
    {
        Id = task.Id;
        QuadrantId = task.QuadrantId;
        Title = task.Title;
        DueAt = task.DueAt;
        ReminderAt = task.ReminderAt;
        EditCommand = editCommand;
        CompleteCommand = completeCommand;
        DeleteCommand = deleteCommand;
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

    public ICommand EditCommand { get; }

    public ICommand CompleteCommand { get; }

    public ICommand DeleteCommand { get; }
}
