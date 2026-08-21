using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public partial class TodayPageViewModel : ObservableObject, IDisposable
{
    private readonly ITodayQueryService todayQueryService;
    private readonly IAppChangeHub appChangeHub;
    private IDisposable? subscription;
    private CancellationTokenSource? cancellation;
    private SynchronizationContext? synchronizationContext;
    private int requestGeneration;

    public TodayPageViewModel(ITodayQueryService todayQueryService, IAppChangeHub appChangeHub)
    {
        this.todayQueryService = todayQueryService;
        this.appChangeHub = appChangeHub;
    }

    public ObservableCollection<TaskItem> Overdue { get; } = [];
    public ObservableCollection<TaskItem> PlannedToday { get; } = [];
    public ObservableCollection<TaskItem> DueToday { get; } = [];
    public ObservableCollection<TaskItem> NeedsReschedule { get; } = [];
    public int UniqueTaskCount { get; private set; }
    public long EstimatedMinutesTotal { get; private set; }
    public bool HasOverdue => Overdue.Count > 0;
    public bool HasPlannedToday => PlannedToday.Count > 0;
    public bool HasDueToday => DueToday.Count > 0;
    public bool HasNeedsReschedule => NeedsReschedule.Count > 0;
    public bool IsEmpty => !IsLoading && !HasError && UniqueTaskCount == 0;
    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    [ObservableProperty] public partial bool IsLoading { get; private set; }
    [ObservableProperty] public partial string? ErrorMessage { get; private set; }

    public async Task ActivateAsync()
    {
        synchronizationContext = SynchronizationContext.Current;
        subscription ??= appChangeHub.Subscribe(OnChange);
        await LoadAsync();
    }

    public async Task LoadAsync()
    {
        cancellation?.Cancel(); cancellation?.Dispose();
        cancellation = new CancellationTokenSource();
        var token = cancellation.Token;
        var generation = ++requestGeneration;
        IsLoading = true; ErrorMessage = null;
        try
        {
            var snapshot = await todayQueryService.GetSnapshotAsync(token);
            if (token.IsCancellationRequested || generation != requestGeneration) return;
            Replace(Overdue, snapshot.Overdue); Replace(PlannedToday, snapshot.PlannedToday);
            Replace(DueToday, snapshot.DueToday); Replace(NeedsReschedule, snapshot.NeedsReschedule);
            UniqueTaskCount = snapshot.UniqueTaskCount; EstimatedMinutesTotal = snapshot.EstimatedMinutesTotal;
            NotifyState();
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch { ErrorMessage = "Today 加载失败，请重试。"; NotifyState(); }
        finally { if (!token.IsCancellationRequested && generation == requestGeneration) IsLoading = false; }
    }

    public void Deactivate()
    {
        cancellation?.Cancel(); cancellation?.Dispose(); cancellation = null;
        subscription?.Dispose(); subscription = null; synchronizationContext = null;
    }
    public void Dispose() => Deactivate();

    private void OnChange(AppChange change)
    {
        if (change.Kind == AppChangeKind.FocusSessionCompleted || synchronizationContext is null) return;
        synchronizationContext.Post(_ => _ = LoadAsync(), null);
    }
    private static void Replace(ObservableCollection<TaskItem> target, IReadOnlyList<TaskItem> source)
    { target.Clear(); foreach (var task in source) target.Add(task); }
    private void NotifyState()
    { OnPropertyChanged(nameof(UniqueTaskCount)); OnPropertyChanged(nameof(EstimatedMinutesTotal)); OnPropertyChanged(nameof(HasOverdue)); OnPropertyChanged(nameof(HasPlannedToday)); OnPropertyChanged(nameof(HasDueToday)); OnPropertyChanged(nameof(HasNeedsReschedule)); OnPropertyChanged(nameof(IsEmpty)); OnPropertyChanged(nameof(HasError)); }
    partial void OnIsLoadingChanged(bool value) => OnPropertyChanged(nameof(IsEmpty));
    partial void OnErrorMessageChanged(string? value) { OnPropertyChanged(nameof(HasError)); OnPropertyChanged(nameof(IsEmpty)); }
}
