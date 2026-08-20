using Quadrant.Core.Enums;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public static class TaskRules
{
    public static string NormalizeTitle(string title)
    {
        ArgumentNullException.ThrowIfNull(title);

        var normalized = title.Trim();
        if (normalized.Length == 0)
        {
            throw new TaskValidationException("Task title cannot be empty.");
        }

        return normalized;
    }

    public static void ValidateQuadrantId(int quadrantId)
    {
        if (quadrantId is < 1 or > 4)
        {
            throw new TaskValidationException("Quadrant ID must be between 1 and 4.");
        }
    }

    public static TaskDraft Validate(TaskDraft draft)
    {
        ArgumentNullException.ThrowIfNull(draft);
        ValidateQuadrantId(draft.QuadrantId);

        return draft with
        {
            Title = NormalizeTitle(draft.Title),
            Note = NormalizeNote(draft.Note)
        };
    }

    public static TaskUpdate Validate(TaskUpdate update)
    {
        ArgumentNullException.ThrowIfNull(update);
        ValidateQuadrantId(update.QuadrantId);

        return update with
        {
            Title = NormalizeTitle(update.Title),
            Note = NormalizeNote(update.Note)
        };
    }

    public static bool IsDueToday(TaskItem task, DateTimeOffset now)
    {
        ArgumentNullException.ThrowIfNull(task);
        return task.DueAt is { } due && due.ToLocalTime().Date == now.ToLocalTime().Date;
    }

    public static bool IsOverdue(TaskItem task, DateTimeOffset now)
    {
        ArgumentNullException.ThrowIfNull(task);
        return !task.IsCompleted && task.DueAt is { } due && due < now;
    }

    public static TaskItem Complete(TaskItem task, DateTimeOffset now)
    {
        ArgumentNullException.ThrowIfNull(task);

        return task with
        {
            IsCompleted = true,
            CompletedAt = now,
            UpdatedAt = now
        };
    }

    public static TaskItem Restore(TaskItem task, DateTimeOffset now)
    {
        ArgumentNullException.ThrowIfNull(task);

        return task with
        {
            IsCompleted = false,
            CompletedAt = null,
            UpdatedAt = now
        };
    }

    public static DateTimeOffset? CalculateReminderAt(
        DateTimeOffset? dueAt,
        ReminderPreset preset,
        DateTimeOffset? customReminderAt = null) =>
        ReminderCalculator.Calculate(preset, dueAt, customReminderAt);

    public static void ValidateReminderAt(DateTimeOffset? reminderAt, DateTimeOffset now)
    {
        if (reminderAt is { } value && value <= now)
        {
            throw new TaskValidationException("提醒时间必须晚于当前时间。");
        }
    }

    private static string? NormalizeNote(string? note)
    {
        var normalized = note?.Trim();
        return string.IsNullOrEmpty(normalized) ? null : normalized;
    }
}
