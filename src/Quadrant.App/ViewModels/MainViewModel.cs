using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public partial class MainViewModel : ObservableObject
{
    private readonly ITaskService taskService;
    private readonly IQuadrantRepository quadrantRepository;

    public MainViewModel(ITaskService taskService, IQuadrantRepository quadrantRepository)
    {
        this.taskService = taskService ?? throw new ArgumentNullException(nameof(taskService));
        this.quadrantRepository = quadrantRepository ?? throw new ArgumentNullException(nameof(quadrantRepository));
    }

    [ObservableProperty]
    private string appTitle = "Quadrant";

    [ObservableProperty]
    private string placeholderTitle = "四象限任务工作区";

    public IReadOnlyList<QuadrantViewModel> Quadrants { get; private set; } = [];

    private IReadOnlyList<TaskItem> loadedTasks = [];

    public event EventHandler? NewTaskRequested;

    public event EventHandler<TaskItem>? EditTaskRequested;

    public event EventHandler<long>? DeleteTaskRequested;

    [RelayCommand]
    private void NewTask() => NewTaskRequested?.Invoke(this, EventArgs.Empty);

    [RelayCommand]
    private void EditTask(long id)
    {
        var task = loadedTasks.FirstOrDefault(item => item.Id == id);
        if (task is not null)
        {
            EditTaskRequested?.Invoke(this, task);
        }
    }

    [RelayCommand]
    private async Task CompleteTask(long id)
    {
        await taskService.SetCompletedAsync(id, true);
        await LoadAsync();
    }

    [RelayCommand]
    private async Task DeleteTask(long id)
    {
        DeleteTaskRequested?.Invoke(this, id);
    }

    public async Task ConfirmedDeleteAsync(long id)
    {
        await taskService.DeleteAsync(id);
        await LoadAsync();
    }

    public async Task CreateAsync(TaskDraft draft)
    {
        await taskService.CreateAsync(draft);
        await LoadAsync();
    }

    public async Task UpdateAsync(TaskUpdate update)
    {
        await taskService.UpdateAsync(update);
        await LoadAsync();
    }

    public async Task LoadAsync(CancellationToken cancellationToken = default)
    {
        var definitions = await quadrantRepository.GetAllAsync(cancellationToken);
        var activeTasks = await taskService.GetActiveAsync(cancellationToken);
        loadedTasks = activeTasks;

        Quadrants = definitions
            .OrderBy(definition => definition.Id)
            .Select(definition => new QuadrantViewModel(
                definition,
                activeTasks.Where(task => task.QuadrantId == definition.Id),
                EditTaskCommand,
                CompleteTaskCommand,
                DeleteTaskCommand))
            .ToArray();

        OnPropertyChanged(nameof(Quadrants));
    }
}
