using System.Globalization;
using System.Windows;
using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;

namespace Quadrant.App.ViewModels;

public partial class TaskEditorViewModel : ObservableObject
{
    private static readonly string[] AcceptedTimeFormats = ["h\\:mm", "hh\\:mm"];
    private readonly IClock clock;

    public TaskEditorViewModel(IEnumerable<QuadrantDefinition> quadrants, IClock clock, TaskItem? task = null)
    {
        Quadrants = quadrants.OrderBy(quadrant => quadrant.Id).ToArray();
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
        IsEdit = task is not null;
        Id = task?.Id;
        Title = task?.Title ?? string.Empty;
        QuadrantId = task?.QuadrantId ?? Quadrants.FirstOrDefault()?.Id ?? 1;
        Note = task?.Note ?? string.Empty;

        if (task?.DueAt is { } due)
        {
            var localDue = due.ToLocalTime();
            DueDate = localDue.Date;
            DueTimeText = localDue.TimeOfDay == new TimeSpan(23, 59, 0)
                ? string.Empty
                : localDue.ToString("H:mm", CultureInfo.InvariantCulture);
        }

        ReminderPreset = InferReminderPreset(task?.DueAt, task?.ReminderAt);
        if (task?.ReminderAt is { } reminder)
        {
            var localReminder = reminder.ToLocalTime();
            CustomReminderDate = localReminder.Date;
            CustomReminderTimeText = localReminder.ToString("H:mm", CultureInfo.InvariantCulture);
        }

        PropertyChanged += (_, args) =>
        {
            if (args.PropertyName is nameof(Title)
                or nameof(DueDate)
                or nameof(DueTimeText)
                or nameof(ReminderPreset)
                or nameof(CustomReminderDate)
                or nameof(CustomReminderTimeText))
            {
                IsValid = true;
            }
        };
    }

    public IReadOnlyList<QuadrantDefinition> Quadrants { get; }

    public bool IsEdit { get; }

    public string EditorTitle => IsEdit ? "编辑任务" : "新建任务";

    public long? Id { get; }

    [ObservableProperty]
    private string title;

    [ObservableProperty]
    private int quadrantId;

    [ObservableProperty]
    private DateTime? dueDate;

    [ObservableProperty]
    private string dueTimeText = string.Empty;

    [ObservableProperty]
    private string note;

    [ObservableProperty]
    private ReminderPreset reminderPreset;

    [ObservableProperty]
    private DateTime? customReminderDate;

    [ObservableProperty]
    private string customReminderTimeText = string.Empty;

    [ObservableProperty]
    private string? titleError;

    [ObservableProperty]
    private string? dueTimeError;

    [ObservableProperty]
    private string? reminderError;

    [ObservableProperty]
    private bool isValid = true;

    public IEnumerable<string> TimeSuggestions => Enumerable.Range(0, 24 * 4)
        .Select(index => TimeSpan.FromMinutes(index * 15).ToString("h\\:mm", CultureInfo.InvariantCulture));

    public IEnumerable<ReminderPreset> ReminderPresets => DueDate is null
        ? [ReminderPreset.None, ReminderPreset.Custom]
        : Enum.GetValues<ReminderPreset>();

    public bool HasDueDate => DueDate is not null;

    partial void OnDueDateChanged(DateTime? value)
    {
        OnPropertyChanged(nameof(HasDueDate));
        OnPropertyChanged(nameof(ReminderPresets));
        if (value is null && ReminderPreset is not ReminderPreset.None and not ReminderPreset.Custom)
        {
            ReminderPreset = ReminderPreset.None;
        }
    }

    public bool TryBuildDraft(out TaskDraft draft)
    {
        TitleError = string.IsNullOrWhiteSpace(Title) ? "任务名称不能为空。" : null;
        DueTimeError = null;
        ReminderError = null;

        DateTimeOffset? dueAt = null;
        if (DueDate is { } date)
        {
            var time = new TimeSpan(23, 59, 0);
            if (!string.IsNullOrWhiteSpace(DueTimeText))
            {
                if (!TimeOnly.TryParseExact(DueTimeText.Trim(), AcceptedTimeFormats, CultureInfo.InvariantCulture, DateTimeStyles.None, out var parsedTime))
                {
                    DueTimeError = "请输入有效时间，例如 9:05。";
                }
                else
                {
                    time = parsedTime.ToTimeSpan();
                }
            }

            if (DueTimeError is null)
            {
                var localDateTime = date.Date.Add(time);
                var offset = TimeZoneInfo.Local.GetUtcOffset(localDateTime);
                dueAt = new DateTimeOffset(localDateTime, offset);
            }
        }

        DateTimeOffset? reminderAt = null;
        if (ReminderPreset == Quadrant.Core.Enums.ReminderPreset.Custom)
        {
            if (CustomReminderDate is not { } reminderDate ||
                !TimeOnly.TryParseExact(CustomReminderTimeText.Trim(), AcceptedTimeFormats, CultureInfo.InvariantCulture, DateTimeStyles.None, out var reminderTime))
            {
                ReminderError = "请输入自定义提醒日期和时间。";
            }
            else
            {
                var localDateTime = reminderDate.Date.Add(reminderTime.ToTimeSpan());
                reminderAt = new DateTimeOffset(localDateTime, TimeZoneInfo.Local.GetUtcOffset(localDateTime));
            }
        }
        else if (ReminderPreset != ReminderPreset.None)
        {
            if (DueDate is null)
            {
                ReminderError = "相对截止时间提醒需要先设置截止日期。";
            }
            reminderAt = ReminderCalculator.Calculate(ReminderPreset, dueAt, null);
        }

        if (ReminderError is null && reminderAt is { } reminder && reminder <= clock.Now)
        {
            ReminderError = "提醒时间已过去，请改为未来时间。";
        }

        if (TitleError is not null || DueTimeError is not null || ReminderError is not null)
        {
            IsValid = false;
            draft = null!;
            return false;
        }

        IsValid = true;
        draft = new TaskDraft(Title.Trim(), QuadrantId, dueAt, reminderAt, string.IsNullOrWhiteSpace(Note) ? null : Note.Trim());
        return true;
    }

    public bool TryBuildUpdate(out TaskUpdate update)
    {
        if (!TryBuildDraft(out var draft) || Id is not { } id)
        {
            update = null!;
            return false;
        }

        update = new TaskUpdate(id, draft.Title, draft.QuadrantId, draft.DueAt, draft.ReminderAt, draft.Note);
        return true;
    }

    private static ReminderPreset InferReminderPreset(DateTimeOffset? due, DateTimeOffset? reminder)
    {
        if (reminder is null)
        {
            return ReminderPreset.None;
        }

        if (due is { } dueAt)
        {
            if (reminder == dueAt) return ReminderPreset.AtDueTime;
            if (reminder == dueAt.AddMinutes(-10)) return ReminderPreset.TenMinutesBefore;
            if (reminder == dueAt.AddHours(-1)) return ReminderPreset.OneHourBefore;
            if (reminder == dueAt.AddDays(-1)) return ReminderPreset.OneDayBefore;
        }

        return ReminderPreset.Custom;
    }

}
