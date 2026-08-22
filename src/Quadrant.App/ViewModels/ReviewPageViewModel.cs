using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public partial class ReviewPageViewModel : ObservableObject, IDisposable
{
    private readonly IReviewQueryService queryService;
    private readonly IAppChangeHub appChangeHub;
    private IDisposable? subscription;
    private CancellationTokenSource? cancellation;
    private CancellationTokenSource? debounceCancellation;
    private SynchronizationContext? synchronizationContext;
    private int requestGeneration;
    private bool isActive;
    private bool isDirty = true;

    public ReviewPageViewModel(IReviewQueryService queryService, IAppChangeHub appChangeHub, ReviewRange defaultRange = ReviewRange.SevenDays)
    {
        this.queryService = queryService ?? throw new ArgumentNullException(nameof(queryService));
        this.appChangeHub = appChangeHub ?? throw new ArgumentNullException(nameof(appChangeHub));
        SelectedRange = defaultRange;
    }

    public ObservableCollection<RecentCompletion> RecentCompleted { get; } = [];
    public ObservableCollection<DateBucketPoint> CompletedTrend { get; } = [];
    public ObservableCollection<DateBucketPoint> FocusTrend { get; } = [];
    public event EventHandler? TrendDataChanged;
    public Array Ranges { get; } = Enum.GetValues<ReviewRange>();
    public ReviewSummary? Summary { get; private set; }
    public bool HasSummary => Summary is not null;
    public bool HasRecentCompleted => RecentCompleted.Count > 0;
    public bool IsEmpty => !IsLoading && !HasError && Summary is { CompletedTaskCount: 0, ProductiveFocusSessionCount: 0 };
    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);
    public string CompletedTasksText => Summary?.CompletedTaskCount.ToString() ?? "—";
    public string FocusTimeText => FormatDuration(Summary?.TotalFocusSeconds ?? 0);
    public string SessionCountText => Summary?.ProductiveFocusSessionCount.ToString() ?? "—";
    public string AverageFocusText => Summary is { HasFocusData: true } summary ? FormatDuration(summary.AverageFocusSeconds) : "无数据";
    public string CurrentInboxText => Summary?.CurrentInboxCount.ToString() ?? "—";
    public string CurrentOverdueText => Summary?.CurrentOverdueCount.ToString() ?? "—";

    [ObservableProperty] public partial ReviewRange SelectedRange { get; set; }
    [ObservableProperty] public partial bool IsLoading { get; private set; }
    [ObservableProperty] public partial string? ErrorMessage { get; private set; }

    public async Task ActivateAsync()
    {
        synchronizationContext = SynchronizationContext.Current;
        isActive = true;
        subscription ??= appChangeHub.Subscribe(OnAppChange);
        await LoadAsync();
    }

    public async Task LoadAsync()
    {
        cancellation?.Cancel(); cancellation?.Dispose();
        cancellation = new CancellationTokenSource(); var token = cancellation.Token; var generation = ++requestGeneration;
        IsLoading = true; ErrorMessage = null;
        try
        {
            var summaryTask = queryService.GetSummaryAsync(SelectedRange, token);
            var recentTask = queryService.GetRecentCompletedAsync(20, token);
            var completedTrendTask = queryService.GetCompletedTrendAsync(SelectedRange, DayOfWeek.Monday, token);
            var focusTrendTask = queryService.GetFocusTrendAsync(SelectedRange, DayOfWeek.Monday, token);
            await Task.WhenAll(summaryTask, recentTask, completedTrendTask, focusTrendTask);
            if (token.IsCancellationRequested || generation != requestGeneration || !isActive) return;
            Summary = summaryTask.Result;
            RecentCompleted.Clear(); foreach (var item in recentTask.Result) RecentCompleted.Add(item);
            CompletedTrend.Clear(); foreach (var item in completedTrendTask.Result) CompletedTrend.Add(item);
            FocusTrend.Clear(); foreach (var item in focusTrendTask.Result) FocusTrend.Add(item);
            isDirty = false; NotifyState(); TrendDataChanged?.Invoke(this, EventArgs.Empty);
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch { ErrorMessage = "Review 加载失败，请重试。"; NotifyState(); }
        finally { if (!token.IsCancellationRequested && generation == requestGeneration) IsLoading = false; }
    }

    public void Deactivate()
    {
        isActive = false; isDirty = true;
        cancellation?.Cancel(); cancellation?.Dispose(); cancellation = null;
        debounceCancellation?.Cancel(); debounceCancellation?.Dispose(); debounceCancellation = null;
        subscription?.Dispose(); subscription = null; synchronizationContext = null;
    }

    public void Dispose() => Deactivate();

    partial void OnSelectedRangeChanged(ReviewRange value)
    {
        if (isActive) _ = LoadAsync();
    }

    partial void OnIsLoadingChanged(bool value) => OnPropertyChanged(nameof(IsEmpty));
    partial void OnErrorMessageChanged(string? value) { OnPropertyChanged(nameof(HasError)); OnPropertyChanged(nameof(IsEmpty)); }

    private void OnAppChange(AppChange change)
    {
        if (change.Kind is not (AppChangeKind.TaskCompleted or AppChangeKind.TaskReopened or AppChangeKind.TaskDeleted or AppChangeKind.FocusSessionCompleted)) return;
        isDirty = true;
        if (!isActive || synchronizationContext is null) return;
        synchronizationContext.Post(_ => DebounceReload(), null);
    }

    private void DebounceReload()
    {
        debounceCancellation?.Cancel(); debounceCancellation?.Dispose();
        debounceCancellation = new CancellationTokenSource(); var token = debounceCancellation.Token;
        _ = ReloadAfterDebounceAsync(token);
    }

    private async Task ReloadAfterDebounceAsync(CancellationToken token)
    {
        try { await Task.Delay(TimeSpan.FromMilliseconds(120), token); if (!token.IsCancellationRequested && isActive && isDirty) await LoadAsync(); }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
    }

    private void NotifyState()
    {
        OnPropertyChanged(nameof(Summary)); OnPropertyChanged(nameof(HasSummary)); OnPropertyChanged(nameof(HasRecentCompleted));
        OnPropertyChanged(nameof(CompletedTasksText)); OnPropertyChanged(nameof(FocusTimeText)); OnPropertyChanged(nameof(SessionCountText));
        OnPropertyChanged(nameof(AverageFocusText)); OnPropertyChanged(nameof(CurrentInboxText)); OnPropertyChanged(nameof(CurrentOverdueText)); OnPropertyChanged(nameof(IsEmpty));
    }

    private static string FormatDuration(long seconds)
    {
        var duration = TimeSpan.FromSeconds(seconds);
        return duration.TotalHours >= 1 ? $"{(int)duration.TotalHours} 小时 {duration.Minutes} 分" : $"{duration.Minutes} 分";
    }
}
