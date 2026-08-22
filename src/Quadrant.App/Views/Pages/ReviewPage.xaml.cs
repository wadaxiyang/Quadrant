using System.Windows;
using System.Windows.Controls;
using Quadrant.App.ViewModels;
using ScottPlot.WPF;
using Wpf.Ui.Appearance;

namespace Quadrant.App.Views.Pages;

public partial class ReviewPage : Page
{
    private ReviewPageViewModel? viewModel;
    private readonly IChartThemeService chartThemeService = new WpfUiChartThemeService();
    private bool chartEventsAttached;
    public ReviewPage() => InitializeComponent();
    private async void Page_Loaded(object sender, RoutedEventArgs e)
    {
        if (viewModel is null && DataContext is MainViewModel main)
        {
            viewModel = new ReviewPageViewModel(main.ReviewQueryService ?? throw new InvalidOperationException("Review query service is unavailable."), main.AppChangeHub);
            DataContext = viewModel;
        }
        AttachChartEvents();
        if (viewModel is not null) await viewModel.ActivateAsync();
        RenderTrendCharts();
    }
    private void Page_Unloaded(object sender, RoutedEventArgs e)
    {
        CompletedTrendPlot.Plot.Clear(); FocusTrendPlot.Plot.Clear();
        DetachChartEvents();
        viewModel?.Deactivate();
    }
    private async void Retry_Click(object sender, RoutedEventArgs e) { if (viewModel is not null) await viewModel.LoadAsync(); }
    private void ViewModel_TrendDataChanged(object? sender, EventArgs e) => RenderTrendCharts();
    private void ApplicationThemeManager_Changed(ApplicationTheme theme, System.Windows.Media.Color accent) => RenderTrendCharts();
    private void AttachChartEvents()
    {
        if (chartEventsAttached || viewModel is null) return;
        viewModel.TrendDataChanged += ViewModel_TrendDataChanged;
        ApplicationThemeManager.Changed += ApplicationThemeManager_Changed;
        chartEventsAttached = true;
    }
    private void DetachChartEvents()
    {
        if (!chartEventsAttached) return;
        if (viewModel is not null) viewModel.TrendDataChanged -= ViewModel_TrendDataChanged;
        ApplicationThemeManager.Changed -= ApplicationThemeManager_Changed;
        chartEventsAttached = false;
    }
    private void RenderTrendCharts()
    {
        RenderTrend(CompletedTrendPlot, viewModel?.CompletedTrend ?? [], "{0:0}");
        RenderTrend(FocusTrendPlot, viewModel?.FocusTrend ?? [], "{0:0} 分", value => value / 60d);
    }

    private void RenderTrend(WpfPlot control, IReadOnlyList<Quadrant.Core.Models.DateBucketPoint> points, string valueFormat, Func<long, double>? transform = null)
    {
        control.Plot.Clear(); chartThemeService.Apply(control.Plot);
        control.UserInputProcessor.Disable();
        if (points.Count == 0 || points.All(point => point.Value == 0)) { control.Visibility = Visibility.Collapsed; return; }
        control.Visibility = Visibility.Visible;
        var bars = control.Plot.Add.Bars(points.Select(point => transform?.Invoke(point.Value) ?? point.Value).ToArray());
        foreach (var bar in bars.Bars) bar.Label = string.Format(valueFormat, bar.Value);
        control.Plot.Axes.Bottom.SetTicks(Enumerable.Range(0, points.Count).Select(index => (double)index).ToArray(), points.Select(point => point.LabelKey).ToArray());
        control.Plot.Axes.Margins(bottom: 0);
        control.Refresh();
    }
}
