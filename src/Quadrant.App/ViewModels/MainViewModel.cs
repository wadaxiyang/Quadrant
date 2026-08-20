using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Enums;
using Quadrant.Core.Services;
using System.Collections.ObjectModel;

namespace Quadrant.App.ViewModels;

public partial class MainViewModel : ObservableObject
{
    private readonly ITaskService taskService;
    private readonly IQuadrantRepository quadrantRepository;
    private readonly IClock clock;

    public MainViewModel(ITaskService taskService, IQuadrantRepository quadrantRepository, IClock clock)
    {
        this.taskService = taskService ?? throw new ArgumentNullException(nameof(taskService));
        this.quadrantRepository = quadrantRepository ?? throw new ArgumentNullException(nameof(quadrantRepository));
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
    }

    [ObservableProperty]
    private string appTitle = "Quadrant";

    [ObservableProperty]
    private string placeholderTitle = "四象限任务工作区";

    [ObservableProperty]
    private TaskFilter selectedFilter = TaskFilter.All;

    [ObservableProperty]
    private string searchText = string.Empty;

    public IReadOnlyList<QuadrantViewModel> Quadrants { get; private set; } = [];

    private IReadOnlyList<TaskItem> loadedTasks = [];
    private IReadOnlyList<QuadrantDefinition> loadedDefinitions = [];

    public ObservableCollection<CompletedTaskViewModel> CompletedTasks { get; } = [];

    public string SearchPlaceholder => "搜索标题或备注";

    public IClock Clock => clock;

    public event EventHandler? NewTaskRequested;

    public event EventHandler<TaskItem>? EditTaskRequested;

    public event EventHandler<long>? DeleteTaskRequested;

    partial void OnSelectedFilterChanged(TaskFilter value) => RebuildQuadrants();

    partial void OnSearchTextChanged(string value) => RebuildQuadrants();

    [RelayCommand]
    private void SelectFilter(TaskFilter filter) => SelectedFilter = filter;

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
    private async Task MoveTask(MoveTaskRequest request)
    {
        await taskService.MoveTaskAsync(request.TaskId, request.TargetQuadrantId);
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

    public async Task LoadCompletedAsync(CancellationToken cancellationToken = default)
    {
        var completed = await taskService.GetCompletedAsync(cancellationToken);
        CompletedTasks.Clear();
        foreach (var task in completed.OrderByDescending(task => task.CompletedAt))
        {
            CompletedTasks.Add(new CompletedTaskViewModel(task, RestoreCompletedCommand, PermanentlyDeleteCompletedCommand));
        }
    }

    [RelayCommand]
    private async Task RestoreCompleted(long id)
    {
        await taskService.SetCompletedAsync(id, false);
        await LoadAsync();
        await LoadCompletedAsync();
    }

    [RelayCommand]
    private async Task PermanentlyDeleteCompleted(long id)
    {
        await taskService.DeleteAsync(id);
        await LoadCompletedAsync();
    }

    public async Task LoadAsync(CancellationToken cancellationToken = default)
    {
        loadedDefinitions = (await quadrantRepository.GetAllAsync(cancellationToken))
            .OrderBy(definition => definition.Id)
            .ToArray();
        loadedTasks = await taskService.GetActiveAsync(cancellationToken);
        RebuildQuadrants();
    }

    private void RebuildQuadrants()
    {
        var query = loadedTasks.AsEnumerable();
        query = SelectedFilter switch
        {
            TaskFilter.Today => query.Where(task => TaskRules.IsDueToday(task, clock.Now)),
            TaskFilter.Overdue => query.Where(task => TaskRules.IsOverdue(task, clock.Now)),
            _ => query
        };

        var search = SearchText.Trim();
        if (search.Length > 0)
        {
            query = query.Where(task =>
                task.Title.Contains(search, StringComparison.CurrentCultureIgnoreCase) ||
                (task.Note?.Contains(search, StringComparison.CurrentCultureIgnoreCase) ?? false));
        }

        Quadrants = loadedDefinitions
            .Select(definition => new QuadrantViewModel(
                definition,
                query.Where(task => task.QuadrantId == definition.Id),
                EditTaskCommand,
                CompleteTaskCommand,
                DeleteTaskCommand,
                clock.Now))
            .ToArray();

        OnPropertyChanged(nameof(Quadrants));
    }
}

public sealed record MoveTaskRequest(long TaskId, int TargetQuadrantId);
