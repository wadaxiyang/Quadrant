using System.Collections.ObjectModel;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public sealed class QuadrantViewModel
{
    public QuadrantViewModel(QuadrantDefinition definition, IEnumerable<TaskItem> tasks)
    {
        Id = definition.Id;
        Name = definition.Name;
        Subtitle = definition.Subtitle;
        Tasks = new ObservableCollection<TaskCardViewModel>(
            tasks
                .OrderBy(task => task.DueAt is null)
                .ThenBy(task => task.DueAt)
                .ThenBy(task => task.CreatedAt)
                .Select(task => new TaskCardViewModel(task)));
    }

    public int Id { get; }

    public string Name { get; }

    public string Subtitle { get; }

    public ObservableCollection<TaskCardViewModel> Tasks { get; }

    public string EmptyText => "暂无任务";

    public string TaskCountText => Tasks.Count.ToString();

    public bool IsEmpty => Tasks.Count == 0;
}
