using Quadrant.Core.Models;
using Quadrant.Core.Enums;
using System.Windows.Input;

namespace Quadrant.App.ViewModels;

public sealed class TaskCardViewModel
{
    private readonly TimeZoneInfo timeZone;
    private readonly DateOnly localToday;

    public TaskCardViewModel(
        TaskItem task,
        ICommand editCommand,
        ICommand editRecurrenceCommand,
        ICommand completeCommand,
        ICommand focusCommand,
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
        EditRecurrenceCommand = editRecurrenceCommand;
        CompleteCommand = completeCommand;
        FocusCommand = focusCommand;
        DeleteCommand = deleteCommand;
        PlanForTodayCommand = planForTodayCommand;
        RemovePlanCommand = removePlanCommand;
        PlannedDate = task.PlannedDate;
        EstimatedMinutes = task.EstimatedMinutes;
        RecurrenceKind = task.RecurrenceKind;
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

    public RecurrenceKind RecurrenceKind { get; }

    public string DueText => DueAt is { } due
        ? $"截止 {TimeZoneInfo.ConvertTime(due, timeZone):yyyy-MM-dd HH:mm}"
        : "无截止时间";

    public string DueMetadataText => DueAt is { } due
        ? FormatCompactDateTime(TimeZoneInfo.ConvertTime(due, timeZone))
        : string.Empty;

    public string ReminderText => ReminderAt is { } reminder
        ? $"提醒 {TimeZoneInfo.ConvertTime(reminder, timeZone):yyyy-MM-dd HH:mm}"
        : string.Empty;

    public string ReminderMetadataText => ReminderAt is { } reminder
        ? $"提醒 {FormatCompactDateTime(TimeZoneInfo.ConvertTime(reminder, timeZone))}"
        : string.Empty;

    public string PlanText => PlannedDate is { } plannedDate
        ? plannedDate == localToday ? "Today" : $"计划 {plannedDate:yyyy-MM-dd}"
        : string.Empty;

    public string PlanMetadataText => PlannedDate is { } plannedDate
        ? plannedDate == localToday ? "Today" : $"{plannedDate:MM-dd} 计划"
        : string.Empty;

    public string EstimateText => EstimatedMinutes is { } estimate ? $"预计 {estimate} 分钟" : string.Empty;

    public string EstimateMetadataText => EstimatedMinutes is { } estimate ? $"{estimate} 分钟" : string.Empty;

    public string RecurrenceText => RecurrenceKind switch
    {
        RecurrenceKind.Daily => "每天重复",
        RecurrenceKind.Weekly => "每周重复",
        RecurrenceKind.Monthly => "每月重复",
        _ => string.Empty
    };

    public string RecurrenceMetadataText => RecurrenceKind switch
    {
        RecurrenceKind.Daily => "每天",
        RecurrenceKind.Weekly => "每周",
        RecurrenceKind.Monthly => "每月",
        _ => string.Empty
    };

    public bool HasDue => DueAt is not null;

    public bool HasMetadata => HasDue || ReminderAt is not null || PlannedDate is not null || EstimatedMinutes is not null || RecurrenceKind != RecurrenceKind.None;

    public bool IsPlannedForToday => PlannedDate == localToday;

    public string AutomationName
    {
        get
        {
            var metadata = new[] { HasDue ? DueText : string.Empty, DueStatusText, ReminderText, PlanText, EstimateText, RecurrenceText }
                .Where(value => value.Length > 0);
            var metadataText = string.Join("；", metadata);
            return metadataText.Length == 0 ? Title : $"{Title}；{metadataText}";
        }
    }

    public bool IsOverdue { get; }

    public string DueStatusText { get; }

    public ICommand EditCommand { get; }

    public ICommand EditRecurrenceCommand { get; }

    public ICommand CompleteCommand { get; }

    public ICommand FocusCommand { get; }

    public ICommand DeleteCommand { get; }

    public ICommand PlanForTodayCommand { get; }

    public ICommand RemovePlanCommand { get; }

    private string FormatCompactDateTime(DateTimeOffset value) =>
        DateOnly.FromDateTime(value.Date) == localToday ? $"今天 {value:HH:mm}" : $"{value:MM-dd HH:mm}";
}
