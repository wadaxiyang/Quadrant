using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public partial class InboxPageViewModel : ObservableObject, IDisposable
{
    private readonly ITaskService taskService;
    private readonly IAppChangeHub appChangeHub;
    private IDisposable? subscription;
    private CancellationTokenSource? loadCancellation;
    private SynchronizationContext? synchronizationContext;

    public InboxPageViewModel(ITaskService taskService, IAppChangeHub appChangeHub)
    {
        this.taskService = taskService ?? throw new ArgumentNullException(nameof(taskService));
        this.appChangeHub = appChangeHub ?? throw new ArgumentNullException(nameof(appChangeHub));
    }

    public ObservableCollection<TaskItem> Tasks { get; } = [];

    public int Count => Tasks.Count;

    public bool IsEmpty => !IsLoading && !HasError && Tasks.Count == 0;

    [ObservableProperty]
    public partial bool IsLoading { get; private set; }

    [ObservableProperty]
    public partial string? ErrorMessage { get; private set; }

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public event EventHandler<RecoverableOperationErrorEventArgs>? RecoverableError;

    partial void OnIsLoadingChanged(bool value) => OnPropertyChanged(nameof(IsEmpty));
    partial void OnErrorMessageChanged(string? value) { OnPropertyChanged(nameof(HasError)); OnPropertyChanged(nameof(IsEmpty)); }

    public async Task ActivateAsync(CancellationToken cancellationToken = default)
    {
        synchronizationContext = SynchronizationContext.Current;
        subscription ??= appChangeHub.Subscribe(OnAppChange);
        await LoadAsync(cancellationToken);
    }

    public async Task LoadAsync(CancellationToken cancellationToken = default)
    {
        loadCancellation?.Cancel();
        loadCancellation?.Dispose();
        loadCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var token = loadCancellation.Token;
        IsLoading = true;
        ErrorMessage = null;
        try
        {
            var tasks = await taskService.GetInboxAsync(cancellationToken: token);
            token.ThrowIfCancellationRequested();
            Tasks.Clear();
            foreach (var task in tasks)
            {
                Tasks.Add(task);
            }
            NotifyCollectionState();
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            ErrorMessage = "Inbox 加载失败，请重试。";
            RecoverableError?.Invoke(this, new RecoverableOperationErrorEventArgs("Inbox 加载失败", exception));
        }
        finally
        {
            if (!token.IsCancellationRequested)
            {
                IsLoading = false;
            }
        }
    }

    public async Task AssignQuadrantAsync(TaskItem task, int quadrantId, CancellationToken cancellationToken = default)
    {
        try
        {
            await taskService.AssignQuadrantAsync(task.Id, quadrantId, cancellationToken);
            Remove(task.Id);
        }
        catch (Exception exception)
        {
            RecoverableError?.Invoke(this, new RecoverableOperationErrorEventArgs("任务分类失败", exception));
        }
    }

    public async Task CompleteAsync(TaskItem task, CancellationToken cancellationToken = default)
    {
        try
        {
            await taskService.SetCompletedAsync(task.Id, true, cancellationToken);
            Remove(task.Id);
        }
        catch (Exception exception)
        {
            RecoverableError?.Invoke(this, new RecoverableOperationErrorEventArgs("任务完成失败", exception));
        }
    }

    public async Task PlanForTodayAsync(TaskItem task, CancellationToken cancellationToken = default)
    {
        try
        {
            var updated = await taskService.PlanForTodayAsync(task.Id, cancellationToken);
            var index = IndexOf(updated.Id);
            if (index >= 0)
            {
                Tasks[index] = updated;
            }
        }
        catch (Exception exception)
        {
            RecoverableError?.Invoke(this, new RecoverableOperationErrorEventArgs("添加到 Today 失败", exception));
        }
    }

    public async Task DeleteAsync(TaskItem task, CancellationToken cancellationToken = default)
    {
        try
        {
            await taskService.DeleteAsync(task.Id, cancellationToken);
            Remove(task.Id);
        }
        catch (Exception exception)
        {
            RecoverableError?.Invoke(this, new RecoverableOperationErrorEventArgs("任务删除失败", exception));
        }
    }

    public void Deactivate()
    {
        loadCancellation?.Cancel();
        loadCancellation?.Dispose();
        loadCancellation = null;
        subscription?.Dispose();
        subscription = null;
        synchronizationContext = null;
    }

    public void Dispose() => Deactivate();

    private void OnAppChange(AppChange change)
    {
        if (synchronizationContext is null || change.Kind == AppChangeKind.FocusSessionCompleted)
        {
            return;
        }

        synchronizationContext.Post(_ => _ = RefreshChangedTaskAsync(change.TaskId), null);
    }

    private async Task RefreshChangedTaskAsync(long taskId)
    {
        try
        {
            var task = await taskService.GetByIdAsync(taskId);
            var index = IndexOf(taskId);
            if (task is null || task.IsCompleted || task.QuadrantId is not null)
            {
                if (index >= 0) Remove(taskId);
                return;
            }

            if (index >= 0)
            {
                Tasks[index] = task;
            }
            else
            {
                Tasks.Add(task);
            }

            SortByCapturedTime();
            NotifyCollectionState();
        }
        catch (Exception exception)
        {
            RecoverableError?.Invoke(this, new RecoverableOperationErrorEventArgs("Inbox 更新失败", exception));
        }
    }

    private void Remove(long id)
    {
        var index = IndexOf(id);
        if (index >= 0) Tasks.RemoveAt(index);
        NotifyCollectionState();
    }

    private int IndexOf(long id)
    {
        for (var index = 0; index < Tasks.Count; index++)
        {
            if (Tasks[index].Id == id) return index;
        }

        return -1;
    }

    private void SortByCapturedTime()
    {
        var ordered = Tasks.OrderBy(task => task.CreatedAt).ThenBy(task => task.Id).ToArray();
        if (ordered.SequenceEqual(Tasks)) return;
        Tasks.Clear();
        foreach (var task in ordered) Tasks.Add(task);
    }

    private void NotifyCollectionState()
    {
        OnPropertyChanged(nameof(Count));
        OnPropertyChanged(nameof(IsEmpty));
    }
}
