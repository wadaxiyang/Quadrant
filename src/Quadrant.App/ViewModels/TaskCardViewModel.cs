using Quadrant.Core.Models;
using System.Windows.Input;

namespace Quadrant.App.ViewModels;

public sealed class TaskCardViewModel
{
    private readonly TimeZoneInfo timeZone;
    private readonly DateOnly localToday;

    public TaskCardViewModel(
        TaskItem task,
        ICommand editCommand,
        ICommand completeCommand,
        ICommand deleteCommand,
        ICommand planForTodayCommand,
        ICommand removePlanCommand,
        DateTimeOffset now,
        TimeZoneInfo timeZone)
    {
        this.timeZone = timeZone ?? throw new ArgumentNullException(nameof(timeZone));
        localToday = DateOnly.FromDateTime(now.Date);
        Id = task.Id;
        QuadrantId = task.QuadrantId;
        Title = task.Title;
        DueAt = task.DueAt;
        ReminderAt = task.ReminderAt;
        EditCommand = editCommand;
        CompleteCommand = completeCommand;
        DeleteCommand = deleteCommand;
        PlanForTodayCommand = planForTodayCommand;
        RemovePlanCommand = removePlanCommand;
        PlannedDate = task.PlannedDate;
        EstimatedMinutes = task.EstimatedMinutes;
        IsOverdue = task.DueAt is { } due && !task.IsCompleted && due < now;
        DueStatusText = IsOverdue
            ? "已逾期"
            : task.DueAt is { } dueAt && DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(dueAt, this.timeZone).Date) == localToday
                ? "今天"
                : string.Empty;
    }

    public long Id { get; }

    public int? QuadrantId { get; }

    public string Title { get; }

    public DateTimeOffset? DueAt { get; }

    public DateTimeOffset? ReminderAt { get; }

    public DateOnly? PlannedDate { get; }

    public int? EstimatedMinutes { get; }

    public string DueText => DueAt is { } due
        ? $"截止 {TimeZoneInfo.ConvertTime(due, timeZone):yyyy-MM-dd HH:mm}"
        : "无截止时间";

    public string ReminderText => ReminderAt is { } reminder
        ? $"提醒 {TimeZoneInfo.ConvertTime(reminder, timeZone):yyyy-MM-dd HH:mm}"
        : string.Empty;

    public string PlanText => PlannedDate is { } plannedDate
        ? plannedDate == localToday ? "Today" : $"计划 {plannedDate:yyyy-MM-dd}"
        : string.Empty;

    public string EstimateText => EstimatedMinutes is { } estimate ? $"预计 {estimate} 分钟" : string.Empty;

    public bool IsOverdue { get; }

    public string DueStatusText { get; }

    public ICommand EditCommand { get; }

    public ICommand CompleteCommand { get; }

    public ICommand DeleteCommand { get; }

    public ICommand PlanForTodayCommand { get; }

    public ICommand RemovePlanCommand { get; }
}
