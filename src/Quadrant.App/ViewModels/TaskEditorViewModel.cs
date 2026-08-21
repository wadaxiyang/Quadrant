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
    private readonly TimeZoneInfo timeZone;
    private readonly DateTimeOffset? originalReminderAt;
    private readonly bool allowInbox;
    private readonly RecurrenceKind originalRecurrenceKind;
    private readonly int originalRecurrenceInterval;
    private readonly string? originalRecurrenceSeriesId;
    private readonly int? originalRecurrenceAnchorDay;
    private readonly DateOnly? originalDueDate;
    private readonly DateOnly? originalPlannedDate;
    private string? recurrenceSeriesId;

    public TaskEditorViewModel(
        IEnumerable<QuadrantDefinition> quadrants,
        IClock clock,
        TaskItem? task = null,
        TimeZoneInfo? timeZone = null,
        bool allowInbox = false)
    {
        Quadrants = quadrants.OrderBy(quadrant => quadrant.Id).ToArray();
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
        this.timeZone = timeZone ?? clock.LocalTimeZone;
        this.allowInbox = allowInbox;
        originalReminderAt = task?.ReminderAt;
        originalRecurrenceKind = task?.RecurrenceKind ?? RecurrenceKind.None;
        originalRecurrenceInterval = task?.RecurrenceInterval ?? 1;
        originalRecurrenceSeriesId = task?.RecurrenceSeriesId;
        originalRecurrenceAnchorDay = task?.RecurrenceAnchorDay;
        originalDueDate = task?.DueAt is { } originalDue ? DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(originalDue, this.timeZone).Date) : null;
        originalPlannedDate = task?.PlannedDate;
        recurrenceSeriesId = task?.RecurrenceSeriesId;
        IsEdit = task is not null;
        Id = task?.Id;
        Title = task?.Title ?? string.Empty;
        QuadrantId = task is not null
            ? task.QuadrantId
            : allowInbox ? null : Quadrants.FirstOrDefault()?.Id ?? 1;
        Note = task?.Note ?? string.Empty;
        PlannedDate = task?.PlannedDate?.ToDateTime(TimeOnly.MinValue);
        EstimatedMinutesText = task?.EstimatedMinutes?.ToString(CultureInfo.InvariantCulture) ?? string.Empty;
        RecurrenceKind = task?.RecurrenceKind ?? RecurrenceKind.None;

        if (task?.DueAt is { } due)
        {
            var localDue = TimeZoneInfo.ConvertTime(due, this.timeZone);
            DueDate = localDue.Date;
            DueTimeText = localDue.TimeOfDay == new TimeSpan(23, 59, 0)
                ? string.Empty
                : localDue.ToString("H:mm", CultureInfo.InvariantCulture);
        }

        ReminderPreset = InferReminderPreset(task?.DueAt, task?.ReminderAt);
        if (task?.ReminderAt is { } reminder)
        {
            var localReminder = TimeZoneInfo.ConvertTime(reminder, this.timeZone);
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
                or nameof(CustomReminderTimeText)
                or nameof(PlannedDate)
                or nameof(EstimatedMinutesText))
            {
                IsValid = true;
            }
        };
    }

    public IReadOnlyList<QuadrantDefinition> Quadrants { get; }

    public string QuadrantLabel => QuadrantId is null
        ? "Inbox（未分类）"
        : Quadrants.FirstOrDefault(quadrant => quadrant.Id == QuadrantId)?.Name ?? $"Q{QuadrantId}";

    public bool IsEdit { get; }

    public string EditorTitle => IsEdit ? "编辑任务" : "新建任务";

    public long? Id { get; }

    [ObservableProperty]
    public partial string Title { get; set; } = string.Empty;

    [ObservableProperty]
    public partial int? QuadrantId { get; set; }

    [ObservableProperty]
    public partial DateTime? DueDate { get; set; }

    [ObservableProperty]
    public partial string DueTimeText { get; set; } = string.Empty;

    [ObservableProperty]
    public partial string Note { get; set; } = string.Empty;

    [ObservableProperty]
    public partial DateTime? PlannedDate { get; set; }

    [ObservableProperty]
    public partial string EstimatedMinutesText { get; set; } = string.Empty;

    [ObservableProperty]
    public partial RecurrenceKind RecurrenceKind { get; set; }

    [ObservableProperty]
    public partial ReminderPreset ReminderPreset { get; set; }

    [ObservableProperty]
    public partial DateTime? CustomReminderDate { get; set; }

    [ObservableProperty]
    public partial string CustomReminderTimeText { get; set; } = string.Empty;

    [ObservableProperty]
    public partial string? TitleError { get; set; }

    [ObservableProperty]
    public partial string? DueTimeError { get; set; }

    [ObservableProperty]
    public partial string? ReminderError { get; set; }

    [ObservableProperty]
    public partial string? PlanningError { get; set; }

    [ObservableProperty]
    public partial bool IsValid { get; set; } = true;

    public IEnumerable<string> TimeSuggestions => Enumerable.Range(0, 24 * 4)
        .Select(index => TimeSpan.FromMinutes(index * 15).ToString("h\\:mm", CultureInfo.InvariantCulture));

    public IEnumerable<ReminderPreset> ReminderPresets => DueDate is null
        ? [ReminderPreset.None, ReminderPreset.Custom]
        : Enum.GetValues<ReminderPreset>();

    public bool HasDueDate => DueDate is not null;

    public IEnumerable<RecurrenceKind> RecurrenceKinds => Enum.GetValues<RecurrenceKind>();

    public string RecurrenceSummary => RecurrenceKind switch
    {
        RecurrenceKind.Daily => "完成后会创建下一项每日任务。",
        RecurrenceKind.Weekly => "完成后会创建下一项每周任务。",
        RecurrenceKind.Monthly => "完成后会创建下一项每月任务；短月会落在当月最后一天。",
        _ => "不重复；完成后仅归档当前任务。"
    };

    partial void OnDueDateChanged(DateTime? value)
    {
        OnPropertyChanged(nameof(HasDueDate));
        OnPropertyChanged(nameof(ReminderPresets));
        if (value is null && ReminderPreset is not ReminderPreset.None and not ReminderPreset.Custom)
        {
            ReminderPreset = ReminderPreset.None;
        }
    }

    partial void OnQuadrantIdChanged(int? value) => OnPropertyChanged(nameof(QuadrantLabel));

    partial void OnRecurrenceKindChanged(RecurrenceKind value) => OnPropertyChanged(nameof(RecurrenceSummary));

    public bool TryBuildDraft(out TaskDraft draft)
    {
        TitleError = string.IsNullOrWhiteSpace(Title) ? "任务名称不能为空。" : null;
        DueTimeError = null;
        ReminderError = null;
        PlanningError = null;
        if (QuadrantId is < 1 or > 4)
        {
            TitleError ??= "请选择有效象限。";
        }
        else if (!allowInbox && QuadrantId is null)
        {
            TitleError ??= "请选择象限。";
        }

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
                if (TryResolveLocalTime(localDateTime, out var resolved, out var error))
                {
                    dueAt = resolved;
                }
                else
                {
                    DueTimeError = error;
                }
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
                if (MatchesOriginalReminderMinute(localDateTime))
                {
                    // Snooze can retain seconds while the editor intentionally shows
                    // minute precision. Preserve the exact stored instant when the
                    // displayed local minute was not changed.
                    reminderAt = originalReminderAt;
                }
                else if (TryResolveLocalTime(localDateTime, out var resolved, out var error))
                {
                    reminderAt = resolved;
                }
                else
                {
                    ReminderError = error;
                }
            }
        }
        else if (ReminderPreset != ReminderPreset.None)
        {
            if (DueDate is null)
            {
                ReminderError = "相对截止时间提醒需要先设置截止日期。";
            }
            reminderAt = ReminderCalculator.Calculate(ReminderPreset, dueAt, null);
            if (reminderAt is { } calculated && MatchesOriginalReminderMinute(calculated.DateTime))
            {
                reminderAt = originalReminderAt;
            }
        }

        if (ReminderError is null && reminderAt is { } reminder && reminder <= clock.LocalNow && reminder != originalReminderAt)
        {
            ReminderError = "提醒时间已过去，请改为未来时间。";
        }

        int? estimatedMinutes = null;
        if (!string.IsNullOrWhiteSpace(EstimatedMinutesText))
        {
            if (!int.TryParse(EstimatedMinutesText.Trim(), NumberStyles.None, CultureInfo.InvariantCulture, out var parsedEstimate) ||
                parsedEstimate is < 1 or > 1440)
            {
                PlanningError = "预计时长需为 1–1440 分钟的整数。";
            }
            else
            {
                estimatedMinutes = parsedEstimate;
            }
        }

        if (TitleError is not null || DueTimeError is not null || ReminderError is not null || PlanningError is not null)
        {
            IsValid = false;
            draft = null!;
            return false;
        }

        IsValid = true;
        var recurrenceAnchorDay = GetRecurrenceAnchorDay(dueAt, PlannedDate is { } planned ? DateOnly.FromDateTime(planned) : null);
        var recurrenceSeries = RecurrenceKind == RecurrenceKind.None
            ? null
            : recurrenceSeriesId ??= Guid.NewGuid().ToString("N");
        draft = new TaskDraft(
            Title.Trim(),
            QuadrantId,
            dueAt,
            reminderAt,
            string.IsNullOrWhiteSpace(Note) ? null : Note.Trim(),
            PlannedDate is { } plannedDate ? DateOnly.FromDateTime(plannedDate) : null,
            estimatedMinutes,
            RecurrenceKind,
            1,
            recurrenceSeries,
            recurrenceAnchorDay);
        return true;
    }

    public bool TryBuildUpdate(out TaskUpdate update)
    {
        if (!TryBuildDraft(out var draft) || Id is not { } id)
        {
            update = null!;
            return false;
        }

        update = new TaskUpdate(id, draft.Title, draft.QuadrantId, draft.DueAt, draft.ReminderAt, draft.Note,
            draft.PlannedDate, draft.EstimatedMinutes, draft.RecurrenceKind, draft.RecurrenceInterval,
            draft.RecurrenceSeriesId, draft.RecurrenceAnchorDay);
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

    private bool TryResolveLocalTime(DateTime localDateTime, out DateTimeOffset value, out string? error)
    {
        if (LocalTimeResolver.TryResolve(localDateTime, timeZone, out value, out var resolutionError))
        {
            error = null;
            return true;
        }

        error = resolutionError switch
        {
            LocalTimeResolutionError.Invalid => "该本地时间因夏令时切换而不存在，请选择其他时间。",
            LocalTimeResolutionError.Ambiguous => "该本地时间因夏令时切换而重复，请选择其他时间。",
            _ => "请输入有效的本地时间。"
        };
        return false;
    }

    private bool MatchesOriginalReminderMinute(DateTime localDateTime)
    {
        if (originalReminderAt is not { } original)
        {
            return false;
        }

        var originalLocal = TimeZoneInfo.ConvertTime(original, timeZone);
        return originalLocal.Year == localDateTime.Year &&
               originalLocal.Month == localDateTime.Month &&
               originalLocal.Day == localDateTime.Day &&
               originalLocal.Hour == localDateTime.Hour &&
               originalLocal.Minute == localDateTime.Minute;
    }

    private int? GetRecurrenceAnchorDay(DateTimeOffset? dueAt, DateOnly? plannedDate)
    {
        if (RecurrenceKind != RecurrenceKind.Monthly)
        {
            return null;
        }

        DateOnly? currentDueDate = dueAt is { } due ? DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(due, timeZone).Date) : null;
        if (currentDueDate != originalDueDate && currentDueDate is { } changedDue)
        {
            return changedDue.Day;
        }

        if (plannedDate != originalPlannedDate && plannedDate is { } changedPlan)
        {
            return changedPlan.Day;
        }

        return originalRecurrenceAnchorDay ?? currentDueDate?.Day ?? plannedDate?.Day ?? clock.LocalDate.Day;
    }

}
