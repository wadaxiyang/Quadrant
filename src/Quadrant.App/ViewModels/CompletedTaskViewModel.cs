using System.Windows.Input;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public sealed class CompletedTaskViewModel
{
    public CompletedTaskViewModel(TaskItem task, ICommand restoreCommand, ICommand deleteCommand)
    {
        Id = task.Id;
        Title = task.Title;
        QuadrantId = task.QuadrantId;
        CompletedText = task.CompletedAt is { } completed
            ? $"完成于 {completed.ToLocalTime():yyyy-MM-dd HH:mm}"
            : "已完成";
        RestoreCommand = restoreCommand;
        DeleteCommand = deleteCommand;
    }

    public long Id { get; }
    public string Title { get; }
    public int? QuadrantId { get; }
    public string CompletedText { get; }
    public ICommand RestoreCommand { get; }
    public ICommand DeleteCommand { get; }
}
