using System.Collections.ObjectModel;
using System.Globalization;
using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;

namespace Quadrant.App.ViewModels;

public enum ReviewActivityMode { Completed, Focus }

public sealed record ReviewMetricBarItem(int? QuadrantId, string Label, string SecondaryLabel, double ProgressValue, string PercentageText, string ValueText);
public sealed record ReviewActivityItem(DateOnly Date, string DisplayLabel, long Value, double RelativeValue, string ValueText, string ToolTipText);
public sealed record ReviewKpiItem(string Label, string Value, string Comparison, bool IsCurrentState = false);
public sealed record ReviewComparisonItem(string MetricName, string CurrentLabel, string PreviousLabel, double CurrentValue, double PreviousValue, string CurrentText, string PreviousText, string DeltaText);
public sealed record ReviewRecentItem(string EventId, string Title, string MetadataText);
public sealed record ReviewRangeChoice(ReviewRange Value, string Label);
public sealed record ReviewActivityChoice(ReviewActivityMode Value, string Label);

public partial class ReviewPageViewModel : ObservableObject, IDisposable
{
    private readonly IReviewQueryService queryService;
    private readonly IAppChangeHub appChangeHub;
    private readonly IReviewInsightService insightService;
    private IDisposable? subscription;
    private CancellationTokenSource? cancellation;
    private CancellationTokenSource? debounceCancellation;
    private SynchronizationContext? synchronizationContext;
    private int requestGeneration;
    private bool isActive;
    private bool isDirty = true;

    public ReviewPageViewModel(IReviewQueryService queryService, IAppChangeHub appChangeHub, ReviewRange defaultRange = ReviewRange.SevenDays, IReviewInsightService? insightService = null)
    {
        this.queryService = queryService ?? throw new ArgumentNullException(nameof(queryService));
        this.appChangeHub = appChangeHub ?? throw new ArgumentNullException(nameof(appChangeHub));
        this.insightService = insightService ?? new ReviewInsightService();
        SelectedRange = defaultRange;
    }

    public Array Ranges { get; } = Enum.GetValues<ReviewRange>();
    public Array ActivityModes { get; } = Enum.GetValues<ReviewActivityMode>();
    public IReadOnlyList<ReviewRangeChoice> RangeChoices { get; } =
    [
        new(ReviewRange.SevenDays, "7 天"), new(ReviewRange.ThirtyDays, "30 天"),
        new(ReviewRange.NinetyDays, "90 天"), new(ReviewRange.AllTime, "全部")
    ];
    public IReadOnlyList<ReviewActivityChoice> ActivityChoices { get; } =
    [
        new(ReviewActivityMode.Completed, "完成"), new(ReviewActivityMode.Focus, "专注")
    ];
    public ObservableCollection<ReviewKpiItem> Kpis { get; } = [];
    public ObservableCollection<ReviewKpiItem> PrimaryKpis { get; } = [];
    public ObservableCollection<ReviewKpiItem> CurrentStateKpis { get; } = [];
    public ObservableCollection<ReviewComparisonItem> Comparisons { get; } = [];
    public ObservableCollection<ReviewMetricBarItem> CompletedQuadrantRows { get; } = [];
    public ObservableCollection<ReviewMetricBarItem> FocusQuadrantRows { get; } = [];
    public ObservableCollection<ReviewActivityItem> ActivityItems { get; } = [];
    public ObservableCollection<ReviewInsight> InsightItems { get; } = [];
    public ObservableCollection<ReviewRecentItem> RecentCompleted { get; } = [];
    public ReviewDashboard? Dashboard { get; private set; }

    public bool HasDashboard => Dashboard is not null;
    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);
    public bool IsEmpty => !IsLoading && !HasError && Dashboard is { Current.CompletedTaskCount: 0, Current.ProductiveFocusSessionCount: 0 };
    public bool HasCompletedData => Dashboard?.Current.CompletedTaskCount > 0;
    public bool HasFocusData => Dashboard?.Current.HasFocusData == true;
    public bool HasPreviousPeriod => Dashboard?.Previous is not null;
    public bool HasInsights => InsightItems.Count > 0;
    public bool IsSevenDayRange => SelectedRange == ReviewRange.SevenDays;
    public bool IsActivityStrip => !IsSevenDayRange;
    public string RangeSubtitle => SelectedRange switch { ReviewRange.SevenDays => "最近 7 天", ReviewRange.ThirtyDays => "最近 30 天", ReviewRange.NinetyDays => "最近 90 天", _ => "全部时间" };
    public string FocusTotalText => FormatDuration(Dashboard?.FocusSummary.TotalFocusSeconds ?? 0);
    public string FocusSessionsText => (Dashboard?.FocusSummary.SessionCount ?? 0).ToString(CultureInfo.CurrentCulture);
    public string FocusAverageText => Dashboard?.FocusSummary.SessionCount > 0 ? FormatDuration(Dashboard.FocusSummary.AverageSessionSeconds) : "—";
    public string FocusLongestText => Dashboard?.FocusSummary.SessionCount > 0 ? FormatDuration(Dashboard.FocusSummary.LongestSessionSeconds) : "—";
    public string MostFocusedTaskTitle => Dashboard?.FocusSummary.MostFocusedTaskTitle ?? "暂无关联任务数据";
    public string MostFocusedTaskDetail => Dashboard?.FocusSummary.MostFocusedTaskTitle is null ? string.Empty : $"{FormatDuration(Dashboard.FocusSummary.MostFocusedTaskSeconds)} · {Dashboard.FocusSummary.MostFocusedTaskSessions} 次";
    public string MostFocusedQuadrantText => Dashboard?.FocusSummary.MostFocusedQuadrantId is { } id ? $"Q{id} · {FormatDuration(Dashboard.FocusSummary.MostFocusedQuadrantSeconds)}" : "暂无象限数据";

    [ObservableProperty] public partial ReviewRange SelectedRange { get; set; }
    [ObservableProperty] public partial ReviewActivityMode SelectedActivityMode { get; set; } = ReviewActivityMode.Completed;
    [ObservableProperty] public partial bool IsLoading { get; private set; }
    [ObservableProperty] public partial string? ErrorMessage { get; private set; }

    public async Task ActivateAsync()
    {
        synchronizationContext = SynchronizationContext.Current;
        isActive = true;
        subscription ??= appChangeHub.Subscribe(OnAppChange);
        if (isDirty || Dashboard is null) await LoadAsync();
    }

    public async Task LoadAsync()
    {
        cancellation?.Cancel();
        cancellation?.Dispose();
        cancellation = new CancellationTokenSource();
        var token = cancellation.Token;
        var generation = ++requestGeneration;
        IsLoading = true;
        ErrorMessage = null;
        try
        {
            var dashboard = await queryService.GetDashboardAsync(SelectedRange, DayOfWeek.Monday, 20, token);
            if (token.IsCancellationRequested || generation != requestGeneration || !isActive) return;
            Dashboard = dashboard;
            Replace(RecentCompleted, dashboard.RecentCompleted.Select(item =>
            {
                var quadrant = item.QuadrantSnapshot is { } id ? $"Q{id}" : "未分类";
                return new ReviewRecentItem(item.EventId, item.TaskTitleSnapshot,
                    $"{quadrant} · {item.CompletedLocalDate.ToString("d", CultureInfo.CurrentCulture)}");
            }));
            BuildKpis(dashboard);
            BuildComparisons(dashboard);
            BuildQuadrantRows(dashboard);
            Replace(InsightItems, insightService.CreateInsights(dashboard));
            BuildActivityItems();
            isDirty = false;
            NotifyDashboardState();
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch
        {
            ErrorMessage = "Review 加载失败，请重试。";
            NotifyDashboardState();
        }
        finally
        {
            if (!token.IsCancellationRequested && generation == requestGeneration) IsLoading = false;
        }
    }

    public void Deactivate()
    {
        isActive = false;
        isDirty = true;
        cancellation?.Cancel(); cancellation?.Dispose(); cancellation = null;
        debounceCancellation?.Cancel(); debounceCancellation?.Dispose(); debounceCancellation = null;
        subscription?.Dispose(); subscription = null; synchronizationContext = null;
    }

    public void Dispose() => Deactivate();

    partial void OnSelectedRangeChanged(ReviewRange value)
    {
        OnPropertyChanged(nameof(IsSevenDayRange));
        OnPropertyChanged(nameof(IsActivityStrip));
        OnPropertyChanged(nameof(RangeSubtitle));
        if (isActive) _ = LoadAsync();
    }

    partial void OnSelectedActivityModeChanged(ReviewActivityMode value) => BuildActivityItems();
    partial void OnIsLoadingChanged(bool value) => OnPropertyChanged(nameof(IsEmpty));
    partial void OnErrorMessageChanged(string? value) { OnPropertyChanged(nameof(HasError)); OnPropertyChanged(nameof(IsEmpty)); }

    private void BuildKpis(ReviewDashboard dashboard)
    {
        var current = dashboard.Current; var previous = dashboard.Previous;
        ReviewKpiItem[] items =
        [
            new("已完成", current.CompletedTaskCount.ToString(CultureInfo.CurrentCulture), FormatCountDelta(current.CompletedTaskCount, previous?.CompletedTaskCount)),
            new("专注时间", FormatDuration(current.TotalFocusSeconds), FormatDurationDelta(current.TotalFocusSeconds, previous?.TotalFocusSeconds)),
            new("专注次数", current.ProductiveFocusSessionCount.ToString(CultureInfo.CurrentCulture), FormatCountDelta(current.ProductiveFocusSessionCount, previous?.ProductiveFocusSessionCount)),
            new("平均专注", current.HasFocusData ? FormatDuration(current.AverageFocusSeconds) : "—", FormatDurationDelta(current.AverageFocusSeconds, previous?.AverageFocusSeconds, current.HasFocusData)),
            new("当前 Inbox", current.CurrentInboxCount.ToString(CultureInfo.CurrentCulture), "当前状态", true),
            new("当前已逾期", current.CurrentOverdueCount.ToString(CultureInfo.CurrentCulture), "当前状态", true)
        ];
        Replace(Kpis, items);
        Replace(PrimaryKpis, items.Take(4));
        Replace(CurrentStateKpis, items.Skip(4));
    }

    private void BuildComparisons(ReviewDashboard dashboard)
    {
        if (dashboard.Previous is not { } previous) { Comparisons.Clear(); return; }
        var completedMax = Math.Max(1, Math.Max(dashboard.Current.CompletedTaskCount, previous.CompletedTaskCount));
        var focusMax = Math.Max(1L, Math.Max(dashboard.Current.TotalFocusSeconds, previous.TotalFocusSeconds));
        Replace(Comparisons,
        [
            new("完成任务", "本期", "上期", dashboard.Current.CompletedTaskCount / (double)completedMax * 100, previous.CompletedTaskCount / (double)completedMax * 100, dashboard.Current.CompletedTaskCount.ToString(CultureInfo.CurrentCulture), previous.CompletedTaskCount.ToString(CultureInfo.CurrentCulture), FormatCountDelta(dashboard.Current.CompletedTaskCount, previous.CompletedTaskCount)),
            new("专注时间", "本期", "上期", dashboard.Current.TotalFocusSeconds / (double)focusMax * 100, previous.TotalFocusSeconds / (double)focusMax * 100, FormatDuration(dashboard.Current.TotalFocusSeconds), FormatDuration(previous.TotalFocusSeconds), FormatDurationDelta(dashboard.Current.TotalFocusSeconds, previous.TotalFocusSeconds))
        ]);
    }

    private void BuildQuadrantRows(ReviewDashboard dashboard)
    {
        var completionTotal = dashboard.CompletedByQuadrant.Sum(value => value.Value);
        var classifiedTotal = dashboard.CompletedByQuadrant.Where(value => value.QuadrantId is not null).Sum(value => value.Value);
        Replace(CompletedQuadrantRows, dashboard.CompletedByQuadrant.Where(value => value.QuadrantId is not null || value.Value > 0).Select(value =>
        {
            var denominator = value.QuadrantId is null ? completionTotal : classifiedTotal;
            var share = denominator == 0 ? 0 : value.Value / (double)denominator;
            return new ReviewMetricBarItem(value.QuadrantId, CompletionLabel(value.QuadrantId), QuadrantSubtitle(value.QuadrantId), share * 100, $"{share:P0}", $"{value.Value} 项");
        }));

        var focusTotal = dashboard.FocusByQuadrant.Sum(value => value.Value);
        var focusMaximum = Math.Max(1, dashboard.FocusByQuadrant.Max(value => value.Value));
        Replace(FocusQuadrantRows, dashboard.FocusByQuadrant.Where(value => value.QuadrantId is not null || value.Value > 0).Select(value =>
        {
            var share = focusTotal == 0 ? 0 : value.Value / (double)focusTotal;
            return new ReviewMetricBarItem(value.QuadrantId, FocusLabel(value.QuadrantId), QuadrantSubtitle(value.QuadrantId), value.Value / (double)focusMaximum * 100, $"{share:P0}", FormatDuration(value.Value));
        }));
    }

    private void BuildActivityItems()
    {
        if (Dashboard is null) { ActivityItems.Clear(); return; }
        var source = SelectedActivityMode == ReviewActivityMode.Completed ? Dashboard.CompletedActivity : Dashboard.FocusActivity;
        var bounded = SelectedRange == ReviewRange.AllTime ? source.TakeLast(36).ToArray() : source.ToArray();
        var maximum = Math.Max(1, bounded.Length == 0 ? 0 : bounded.Max(point => point.Value));
        Replace(ActivityItems, bounded.Select(point =>
        {
            var valueText = SelectedActivityMode == ReviewActivityMode.Completed ? $"{point.Value} 项" : FormatDuration(point.Value);
            return new ReviewActivityItem(point.StartDate, FormatActivityLabel(point.StartDate), point.Value, point.Value / (double)maximum * 100, valueText, $"{point.LabelKey} · {valueText}");
        }));
    }

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
        debounceCancellation = new CancellationTokenSource();
        _ = ReloadAfterDebounceAsync(debounceCancellation.Token);
    }

    private async Task ReloadAfterDebounceAsync(CancellationToken token)
    {
        try { await Task.Delay(TimeSpan.FromMilliseconds(120), token); if (!token.IsCancellationRequested && isActive && isDirty) await LoadAsync(); }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
    }

    private void NotifyDashboardState()
    {
        OnPropertyChanged(nameof(Dashboard)); OnPropertyChanged(nameof(HasDashboard)); OnPropertyChanged(nameof(IsEmpty));
        OnPropertyChanged(nameof(HasCompletedData)); OnPropertyChanged(nameof(HasFocusData)); OnPropertyChanged(nameof(HasPreviousPeriod));
        OnPropertyChanged(nameof(HasInsights));
        OnPropertyChanged(nameof(FocusTotalText)); OnPropertyChanged(nameof(FocusSessionsText)); OnPropertyChanged(nameof(FocusAverageText)); OnPropertyChanged(nameof(FocusLongestText));
        OnPropertyChanged(nameof(MostFocusedTaskTitle)); OnPropertyChanged(nameof(MostFocusedTaskDetail)); OnPropertyChanged(nameof(MostFocusedQuadrantText));
    }

    private static void Replace<T>(ObservableCollection<T> destination, IEnumerable<T> source) { destination.Clear(); foreach (var item in source) destination.Add(item); }
    private static string CompletionLabel(int? id) => id is null ? "未分类" : $"Q{id}";
    private static string FocusLabel(int? id) => id is null ? "未关联" : $"Q{id}";
    private static string QuadrantSubtitle(int? id) => id switch { 1 => "重要且紧急", 2 => "重要不紧急", 3 => "紧急不重要", 4 => "不重要不紧急", _ => "无象限" };
    private static string FormatActivityLabel(DateOnly date) => date.ToString("ddd", CultureInfo.CurrentCulture);
    public static string FormatDuration(long seconds) { var duration = TimeSpan.FromSeconds(Math.Max(0, seconds)); return duration.TotalHours >= 1 ? $"{(int)duration.TotalHours} 小时 {duration.Minutes:00} 分" : $"{duration.Minutes} 分"; }
    private static string FormatCountDelta(long current, long? previous)
    {
        if (previous is null) return string.Empty;
        var difference = current - previous.Value;
        if (previous == 0) return difference == 0 ? "与上期相同" : $"+{difference}";
        return difference == 0 ? "与上期相同" : $"{(difference > 0 ? "↑" : "↓")} {Math.Abs(difference) / (double)previous.Value:P0}";
    }
    private static string FormatDurationDelta(long current, long? previous, bool hasCurrent = true)
    {
        if (previous is null || !hasCurrent) return string.Empty;
        var difference = current - previous.Value;
        return difference == 0 ? "与上期相同" : $"{(difference > 0 ? "+" : "−")}{FormatDuration(Math.Abs(difference))}";
    }
}
