using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public sealed partial class QuadrantViewModel : ObservableObject
{
    public QuadrantViewModel(QuadrantDefinition definition)
    {
        Id = definition.Id;
        Name = definition.Name;
        Subtitle = definition.Subtitle;
    }

    public int Id { get; }

    [ObservableProperty]
    public partial string Name { get; private set; }

    [ObservableProperty]
    public partial string Subtitle { get; private set; }

    public ObservableCollection<TaskCardViewModel> Tasks { get; } = [];

    public string EmptyText => "暂无任务";

    public string TaskCountText => Tasks.Count.ToString(System.Globalization.CultureInfo.InvariantCulture);

    public bool IsEmpty => Tasks.Count == 0;

    public void UpdateDefinition(QuadrantDefinition definition)
    {
        if (definition.Id != Id)
        {
            throw new ArgumentException("Quadrant ID cannot change.", nameof(definition));
        }

        Name = definition.Name;
        Subtitle = definition.Subtitle;
    }

    public void SynchronizeTasks(IReadOnlyList<TaskCardViewModel> desired)
    {
        var desiredIds = desired.Select(task => task.Id).ToHashSet();
        for (var index = Tasks.Count - 1; index >= 0; index--)
        {
            if (!desiredIds.Contains(Tasks[index].Id))
            {
                Tasks.RemoveAt(index);
            }
        }

        for (var targetIndex = 0; targetIndex < desired.Count; targetIndex++)
        {
            var desiredTask = desired[targetIndex];
            if (targetIndex < Tasks.Count && Tasks[targetIndex].Id == desiredTask.Id)
            {
                if (!ReferenceEquals(Tasks[targetIndex], desiredTask))
                {
                    Tasks[targetIndex] = desiredTask;
                }
                continue;
            }

            var existingIndex = -1;
            for (var index = targetIndex + 1; index < Tasks.Count; index++)
            {
                if (Tasks[index].Id == desiredTask.Id)
                {
                    existingIndex = index;
                    break;
                }
            }

            if (existingIndex >= 0)
            {
                Tasks.Move(existingIndex, targetIndex);
                if (!ReferenceEquals(Tasks[targetIndex], desiredTask))
                {
                    Tasks[targetIndex] = desiredTask;
                }
            }
            else
            {
                Tasks.Insert(targetIndex, desiredTask);
            }
        }

        while (Tasks.Count > desired.Count)
        {
            Tasks.RemoveAt(Tasks.Count - 1);
        }

        OnPropertyChanged(nameof(TaskCountText));
        OnPropertyChanged(nameof(IsEmpty));
    }
}
