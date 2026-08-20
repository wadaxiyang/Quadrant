using System.Globalization;
using System.Windows;
using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public partial class TaskEditorViewModel : ObservableObject
{
    private static readonly string[] AcceptedTimeFormats = ["h\\:mm", "hh\\:mm"];

    public TaskEditorViewModel(IEnumerable<QuadrantDefinition> quadrants, TaskItem? task = null)
    {
        Quadrants = quadrants.OrderBy(quadrant => quadrant.Id).ToArray();
        IsEdit = task is not null;
        Id = task?.Id;
        ExistingReminderAt = task?.ReminderAt;
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

        PropertyChanged += (_, args) =>
        {
            if (args.PropertyName is nameof(Title) or nameof(DueTimeText))
            {
                IsValid = true;
            }
        };
    }

    public IReadOnlyList<QuadrantDefinition> Quadrants { get; }

    public bool IsEdit { get; }

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
    private string? titleError;

    [ObservableProperty]
    private string? dueTimeError;

    [ObservableProperty]
    private bool isValid = true;

    public IEnumerable<string> TimeSuggestions => Enumerable.Range(0, 24 * 4)
        .Select(index => TimeSpan.FromMinutes(index * 15).ToString("h\\:mm", CultureInfo.InvariantCulture));

    public bool TryBuildDraft(out TaskDraft draft)
    {
        TitleError = string.IsNullOrWhiteSpace(Title) ? "请输入任务标题。" : null;
        DueTimeError = null;

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

        if (TitleError is not null || DueTimeError is not null)
        {
            IsValid = false;
            draft = null!;
            return false;
        }

        IsValid = true;
        draft = new TaskDraft(Title.Trim(), QuadrantId, dueAt, Note: string.IsNullOrWhiteSpace(Note) ? null : Note.Trim());
        return true;
    }

    public bool TryBuildUpdate(out TaskUpdate update)
    {
        if (!TryBuildDraft(out var draft) || Id is not { } id)
        {
            update = null!;
            return false;
        }

        update = new TaskUpdate(id, draft.Title, draft.QuadrantId, draft.DueAt, ExistingReminderAt, draft.Note);
        return true;
    }

    private DateTimeOffset? ExistingReminderAt { get; }
}
