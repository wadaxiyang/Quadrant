using Quadrant.Core.Enums;
using Quadrant.Core.Models;
using Quadrant.Core.Services;
using Xunit;

namespace Quadrant.Core.Tests;

public sealed class TaskRulesTests
{
    private static readonly DateTimeOffset Now = new(2026, 8, 20, 12, 0, 0, TimeSpan.FromHours(8));

    [Fact]
    public void Empty_title_is_rejected_after_trimming()
    {
        var draft = new TaskDraft("  ", 1);

        var exception = Assert.Throws<TaskValidationException>(() => TaskRules.Validate(draft));

        Assert.Equal("Task title cannot be empty.", exception.Message);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(5)]
    public void Quadrant_outside_fixed_range_is_rejected(int quadrantId)
    {
        var draft = new TaskDraft("Task", quadrantId);

        Assert.Throws<TaskValidationException>(() => TaskRules.Validate(draft));
    }

    [Fact]
    public void Validation_trims_title_and_converts_blank_note_to_null()
    {
        var result = TaskRules.Validate(new TaskDraft("  Task  ", 2, Note: "  "));

        Assert.Equal("Task", result.Title);
        Assert.Null(result.Note);
    }

    [Fact]
    public void Today_uses_the_current_local_date_at_offset_boundary()
    {
        var dueAt = new DateTimeOffset(2026, 8, 19, 16, 30, 0, TimeSpan.Zero);
        var task = CreateTask(dueAt: dueAt);

        Assert.True(TaskRules.IsDueToday(task, Now));
    }

    [Fact]
    public void Today_excludes_task_after_local_midnight()
    {
        var dueAt = new DateTimeOffset(2026, 8, 21, 0, 0, 0, TimeSpan.FromHours(8));
        var task = CreateTask(dueAt: dueAt);

        Assert.False(TaskRules.IsDueToday(task, Now));
    }

    [Fact]
    public void Today_and_overdue_exclude_tasks_without_due_date()
    {
        var task = CreateTask();

        Assert.False(TaskRules.IsDueToday(task, Now));
        Assert.False(TaskRules.IsOverdue(task, Now));
    }

    [Fact]
    public void Overdue_excludes_completed_tasks_even_when_due_is_past()
    {
        var task = CreateTask(dueAt: Now.AddDays(-1)) with { IsCompleted = true };

        Assert.False(TaskRules.IsOverdue(task, Now));
    }

    [Fact]
    public void Overdue_excludes_completed_tasks()
    {
        var task = CreateTask(dueAt: Now.AddMinutes(-1));

        Assert.True(TaskRules.IsOverdue(task, Now));
        Assert.False(TaskRules.IsOverdue(task with { IsCompleted = true }, Now));
    }

    [Fact]
    public void Complete_sets_completion_time_and_restore_clears_it()
    {
        var task = CreateTask();

        var completed = TaskRules.Complete(task, Now);
        var restored = TaskRules.Restore(completed, Now.AddMinutes(5));

        Assert.True(completed.IsCompleted);
        Assert.Equal(Now, completed.CompletedAt);
        Assert.False(restored.IsCompleted);
        Assert.Null(restored.CompletedAt);
        Assert.Equal(Now.AddMinutes(5), restored.UpdatedAt);
    }

    [Theory]
    [InlineData(ReminderPreset.None, 0)]
    [InlineData(ReminderPreset.AtDueTime, 0)]
    [InlineData(ReminderPreset.TenMinutesBefore, -10)]
    [InlineData(ReminderPreset.OneHourBefore, -60)]
    [InlineData(ReminderPreset.OneDayBefore, -1440)]
    public void Reminder_presets_calculate_from_due_time(ReminderPreset preset, int offsetMinutes)
    {
        var dueAt = Now.AddDays(2);
        var reminderAt = TaskRules.CalculateReminderAt(dueAt, preset);

        if (preset == ReminderPreset.None)
        {
            Assert.Null(reminderAt);
        }
        else
        {
            Assert.Equal(dueAt.AddMinutes(offsetMinutes), reminderAt);
        }
    }

    private static TaskItem CreateTask(DateTimeOffset? dueAt = null) =>
        new(
            1,
            "Task",
            1,
            dueAt,
            null,
            null,
            false,
            null,
            Now.AddHours(-1),
            Now.AddHours(-1));
}
