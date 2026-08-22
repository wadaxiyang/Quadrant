using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;

namespace Quadrant.App.ViewModels;

public partial class MainViewModel : ObservableObject
{
    private readonly ITaskService taskService;
    private readonly IQuadrantRepository quadrantRepository;
    private readonly IClock clock;
    private readonly IAppChangeHub appChangeHub;
    private readonly Dictionary<long, TaskItem> loadedTasks = [];
    private readonly Dictionary<long, TaskCardViewModel> taskCards = [];
    private IReadOnlyList<QuadrantDefinition> loadedDefinitions = [];

    public MainViewModel(ITaskService taskService, IQuadrantRepository quadrantRepository, IClock clock, IAppChangeHub appChangeHub, ITodayQueryService todayQueryService, IFocusTimerService? focusTimerService = null, PomodoroTimerService? pomodoroTimerService = null, IFocusSessionService? focusSessionService = null, IReviewQueryService? reviewQueryService = null, AppSettings? settings = null)
    {
        this.taskService = taskService ?? throw new ArgumentNullException(nameof(taskService));
        this.quadrantRepository = quadrantRepository ?? throw new ArgumentNullException(nameof(quadrantRepository));
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
        this.appChangeHub = appChangeHub ?? throw new ArgumentNullException(nameof(appChangeHub));
        TodayQueryService = todayQueryService ?? throw new ArgumentNullException(nameof(todayQueryService));
        FocusTimerService = focusTimerService ?? throw new ArgumentNullException(nameof(focusTimerService));
        PomodoroTimerService = pomodoroTimerService ?? throw new ArgumentNullException(nameof(pomodoroTimerService));
        FocusSessionService = focusSessionService ?? throw new ArgumentNullException(nameof(focusSessionService));
        ReviewQueryService = reviewQueryService;
        Settings = settings ?? AppSettings.Default;
    }

    [ObservableProperty]
    public partial string AppTitle { get; set; } = "Quadrant";

    [ObservableProperty]
    public partial string PlaceholderTitle { get; set; } = "四象限任务工作区";

    [ObservableProperty]
    public partial TaskFilter SelectedFilter { get; set; } = TaskFilter.All;

    [ObservableProperty]
    public partial string SearchText { get; set; } = string.Empty;

    public IReadOnlyList<QuadrantViewModel> Quadrants { get; private set; } = [];

    public IReadOnlyList<TaskItem> ActiveTasks => loadedTasks.Values.ToArray();

    public ObservableCollection<CompletedTaskViewModel> CompletedTasks { get; } = [];

    [ObservableProperty]
    public partial string PossiblyMissedReminderText { get; private set; } = string.Empty;

    public bool HasPossiblyMissedReminders => PossiblyMissedReminderText.Length > 0;

    public string SearchPlaceholder => "搜索标题或备注";

    public IClock Clock => clock;

    public ITaskService TaskService => taskService;

    public IAppChangeHub AppChangeHub => appChangeHub;

    public ITodayQueryService TodayQueryService { get; }
    public IFocusTimerService FocusTimerService { get; }
    public PomodoroTimerService PomodoroTimerService { get; }
    public IFocusSessionService FocusSessionService { get; }
    public IReviewQueryService? ReviewQueryService { get; }
    public AppSettings Settings { get; private set; }

    public void UpdateSettings(AppSettings settings) => Settings = settings ?? throw new ArgumentNullException(nameof(settings));

    public event EventHandler? NewTaskRequested;
    public event EventHandler<QuadrantTaskRequestEventArgs>? NewTaskInQuadrantRequested;
    public event EventHandler<TaskItem>? EditTaskRequested;
    public event EventHandler<TaskItem>? RepeatTaskRequested;
    public event EventHandler<long>? FocusTaskRequested;
    public event EventHandler<long>? DeleteTaskRequested;
    public event EventHandler<RecoverableOperationErrorEventArgs>? RecoverableError;

    partial void OnSelectedFilterChanged(TaskFilter value) => RebuildQuadrants();
    partial void OnSearchTextChanged(string value) => RebuildQuadrants();
    partial void OnPossiblyMissedReminderTextChanged(string value) => OnPropertyChanged(nameof(HasPossiblyMissedReminders));

    [RelayCommand]
    private void SelectFilter(TaskFilter filter) => SelectedFilter = filter;

    [RelayCommand]
    private void NewTask() => NewTaskRequested?.Invoke(this, EventArgs.Empty);

    [RelayCommand]
    private void NewTaskInQuadrant(int quadrantId)
    {
        if (Quadrants.Any(quadrant => quadrant.Id == quadrantId))
        {
            NewTaskInQuadrantRequested?.Invoke(this, new QuadrantTaskRequestEventArgs(quadrantId));
        }
    }

    [RelayCommand]
    private void EditTask(long id)
    {
        if (loadedTasks.TryGetValue(id, out var task))
        {
            EditTaskRequested?.Invoke(this, task);
        }
    }

    [RelayCommand]
    private void EditRecurrence(long id)
    {
        if (loadedTasks.TryGetValue(id, out var task))
        {
            RepeatTaskRequested?.Invoke(this, task);
        }
    }

    [RelayCommand]
    private async Task CompleteTask(long id)
    {
        try
        {
            await CompleteAndRefreshAsync(id);
        }
        catch (Exception exception)
        {
            ReportRecoverableError("任务完成失败", exception);
        }
    }

    [RelayCommand]
    private Task MoveTask(MoveTaskRequest request) => MoveTaskAsync(request);

    public async Task<TaskItem?> MoveTaskAsync(MoveTaskRequest request, CancellationToken cancellationToken = default)
    {
        try
        {
            var moved = await taskService.MoveTaskAsync(request.TaskId, request.TargetQuadrantId, cancellationToken);
            if (moved is not null && !moved.IsCompleted)
            {
                UpsertActiveTask(moved);
            }

            return moved;
        }
        catch (Exception exception)
        {
            ReportRecoverableError("任务移动失败", exception);
            return null;
        }
    }

    public async Task<TaskItem?> RestoreMovedTaskAsync(
        long taskId,
        int expectedQuadrantId,
        int targetQuadrantId,
        CancellationToken cancellationToken = default)
    {
        try
        {
            var current = await taskService.GetByIdAsync(taskId, cancellationToken);
            if (current is null || current.IsCompleted || current.QuadrantId != expectedQuadrantId)
            {
                return null;
            }

            var restored = await taskService.MoveTaskAsync(taskId, targetQuadrantId, cancellationToken);
            if (restored is not null && !restored.IsCompleted)
            {
                UpsertActiveTask(restored);
            }

            return restored;
        }
        catch (Exception exception)
        {
            ReportRecoverableError("撤销任务移动失败", exception);
            return null;
        }
    }

    [RelayCommand]
    private void FocusTask(long id)
    {
        if (loadedTasks.ContainsKey(id))
        {
            FocusTaskRequested?.Invoke(this, id);
        }
    }

    [RelayCommand]
    private void DeleteTask(long id) => DeleteTaskRequested?.Invoke(this, id);

    [RelayCommand]
    private async Task PlanForToday(long id)
    {
        try
        {
            UpsertActiveTask(await taskService.PlanForTodayAsync(id));
        }
        catch (Exception exception)
        {
            ReportRecoverableError("添加到 Today 失败", exception);
        }
    }

    [RelayCommand]
    private async Task RemovePlan(long id)
    {
        try
        {
            UpsertActiveTask(await taskService.RemovePlanAsync(id));
        }
        catch (Exception exception)
        {
            ReportRecoverableError("移除计划失败", exception);
        }
    }

    public async Task ConfirmedDeleteAsync(long id)
    {
        await taskService.DeleteAsync(id);
        RemoveActiveTask(id);
    }

    public async Task CreateAsync(TaskDraft draft)
    {
        var task = await taskService.CreateAsync(draft);
        if (task.QuadrantId is not null)
        {
            UpsertActiveTask(task);
        }
    }

    public async Task UpdateAsync(TaskUpdate update)
    {
        var task = await taskService.UpdateAsync(update);
        if (task.QuadrantId is null)
        {
            RemoveActiveTask(task.Id);
        }
        else
        {
            UpsertActiveTask(task);
        }
    }

    public async Task OpenTaskAsync(long id, CancellationToken cancellationToken = default)
    {
        var task = await taskService.GetByIdAsync(id, cancellationToken);
        if (task is not null && !task.IsCompleted)
        {
            EditTaskRequested?.Invoke(this, task);
        }
    }

    public async Task RefreshActiveTaskAsync(long id, CancellationToken cancellationToken = default)
    {
        var task = await taskService.GetByIdAsync(id, cancellationToken);
        if (task is null || task.IsCompleted || task.QuadrantId is null)
        {
            RemoveActiveTask(id);
            return;
        }

        UpsertActiveTask(task);
    }

    public async Task CompleteFromNotificationAsync(long id, CancellationToken cancellationToken = default)
    {
        await CompleteAndRefreshAsync(id, cancellationToken);
    }

    public async Task SnoozeFromNotificationAsync(long id, CancellationToken cancellationToken = default)
    {
        var task = await taskService.SnoozeAsync(id, TimeSpan.FromMinutes(10), cancellationToken);
        if (task is not null && !task.IsCompleted)
        {
            UpsertActiveTask(task);
        }
    }

    public void SetPossiblyMissedReminders(IEnumerable<TaskItem> tasks)
    {
        const int displayedTitleLimit = 5;
        var titles = tasks.Select(task => task.Title).Where(title => !string.IsNullOrWhiteSpace(title)).ToArray();
        PossiblyMissedReminderText = titles.Length == 0
            ? string.Empty
            : $"可能错过 {titles.Length} 条提醒：{string.Join("、", titles.Take(displayedTitleLimit))}{(titles.Length > displayedTitleLimit ? "……" : string.Empty)}";
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
        try
        {
            var restored = await taskService.SetCompletedAsync(id, false);
            UpsertActiveTask(restored);
            await LoadCompletedAsync();
        }
        catch (Exception exception)
        {
            ReportRecoverableError("任务恢复失败", exception);
        }
    }

    [RelayCommand]
    private async Task PermanentlyDeleteCompleted(long id)
    {
        try
        {
            await taskService.DeleteAsync(id);
            await LoadCompletedAsync();
        }
        catch (Exception exception)
        {
            ReportRecoverableError("任务删除失败", exception);
        }
    }

    public async Task LoadAsync(CancellationToken cancellationToken = default)
    {
        var definitionsTask = quadrantRepository.GetAllAsync(cancellationToken);
        var tasksTask = taskService.GetActiveAsync(cancellationToken);
        await Task.WhenAll(definitionsTask, tasksTask);

        loadedDefinitions = definitionsTask.Result.OrderBy(definition => definition.Id).ToArray();
        loadedTasks.Clear();
        taskCards.Clear();
        var now = clock.LocalNow;
        foreach (var task in tasksTask.Result)
        {
            loadedTasks[task.Id] = task;
            taskCards[task.Id] = CreateTaskCard(task, now);
        }

        EnsureQuadrants();
        RebuildQuadrants(now);
    }

    public void UpdateDefinitions(IReadOnlyList<QuadrantDefinition> definitions)
    {
        loadedDefinitions = definitions.OrderBy(definition => definition.Id).ToArray();
        EnsureQuadrants();
        RebuildQuadrants();
    }

    private void UpsertActiveTask(TaskItem task)
    {
        loadedTasks[task.Id] = task;
        taskCards[task.Id] = CreateTaskCard(task, clock.LocalNow);
        RebuildQuadrants();
    }

    private async Task<TaskItem> CompleteAndRefreshAsync(long id, CancellationToken cancellationToken = default)
    {
        long? nextTaskId = null;
        using var subscription = appChangeHub.Subscribe(change =>
        {
            if (change.Kind == AppChangeKind.TaskCreated)
            {
                nextTaskId = change.TaskId;
            }
        });
        var completed = await taskService.SetCompletedAsync(id, true, cancellationToken);
        RemoveActiveTask(id);
        if (nextTaskId is { } nextId)
        {
            await RefreshActiveTaskAsync(nextId, cancellationToken);
        }

        return completed;
    }

    private void RemoveActiveTask(long id)
    {
        loadedTasks.Remove(id);
        taskCards.Remove(id);
        RebuildQuadrants();
    }

    private TaskCardViewModel CreateTaskCard(TaskItem task, DateTimeOffset now) =>
        new(task, EditTaskCommand, EditRecurrenceCommand, CompleteTaskCommand, FocusTaskCommand, DeleteTaskCommand, PlanForTodayCommand, RemovePlanCommand, now, clock.LocalTimeZone);

    private void EnsureQuadrants()
    {
        var definitions = loadedDefinitions.OrderBy(definition => definition.Id).ToArray();
        if (Quadrants.Count != definitions.Length ||
            Quadrants.Select(quadrant => quadrant.Id).SequenceEqual(definitions.Select(definition => definition.Id)) is false)
        {
            Quadrants = definitions.Select(definition => new QuadrantViewModel(definition)).ToArray();
            OnPropertyChanged(nameof(Quadrants));
            return;
        }

        for (var index = 0; index < definitions.Length; index++)
        {
            Quadrants[index].UpdateDefinition(definitions[index]);
        }
    }

    private void RebuildQuadrants() => RebuildQuadrants(clock.LocalNow);

    private void RebuildQuadrants(DateTimeOffset now)
    {
        IEnumerable<TaskItem> query = loadedTasks.Values;
        query = SelectedFilter switch
        {
            TaskFilter.Today => query.Where(task => TaskRules.IsDueToday(task, now)),
            TaskFilter.Overdue => query.Where(task => TaskRules.IsOverdue(task, now)),
            _ => query
        };

        var search = SearchText.Trim();
        if (search.Length > 0)
        {
            query = query.Where(task =>
                task.Title.Contains(search, StringComparison.CurrentCultureIgnoreCase) ||
                (task.Note?.Contains(search, StringComparison.CurrentCultureIgnoreCase) ?? false));
        }

        var tasksByQuadrant = query
            .Where(task => task.QuadrantId is not null)
            .GroupBy(task => task.QuadrantId)
            .ToDictionary(
                group => group.Key!.Value,
                group => (IReadOnlyList<TaskCardViewModel>)group
                    .OrderBy(task => task.DueAt is null)
                    .ThenBy(task => task.DueAt)
                    .ThenBy(task => task.CreatedAt)
                    .Select(task => taskCards[task.Id])
                    .ToArray());

        foreach (var quadrant in Quadrants)
        {
            quadrant.SynchronizeTasks(tasksByQuadrant.GetValueOrDefault(quadrant.Id) ?? []);
        }
    }

    private void ReportRecoverableError(string title, Exception exception) =>
        RecoverableError?.Invoke(this, new RecoverableOperationErrorEventArgs(title, exception));
}

public sealed record MoveTaskRequest(long TaskId, int TargetQuadrantId);

public sealed class QuadrantTaskRequestEventArgs(int quadrantId) : EventArgs
{
    public int QuadrantId { get; } = quadrantId;
}

public sealed class RecoverableOperationErrorEventArgs(string title, Exception exception) : EventArgs
{
    public string Title { get; } = title;
    public Exception Exception { get; } = exception;
}
