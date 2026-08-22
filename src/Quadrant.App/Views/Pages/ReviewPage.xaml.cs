using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using Quadrant.App.ViewModels;

namespace Quadrant.App.Views.Pages;

public partial class ReviewPage : Page
{
    private ReviewPageViewModel? viewModel;

    public ReviewPage() => InitializeComponent();

    private async void Page_Loaded(object sender, RoutedEventArgs e)
    {
        if (viewModel is null && DataContext is MainViewModel main)
        {
            viewModel = new ReviewPageViewModel(
                main.ReviewQueryService ?? throw new InvalidOperationException("Review query service is unavailable."),
                main.AppChangeHub,
                main.Settings.ReviewDefaultRange,
                weekStart: main.Settings.WeekStart);
            DataContext = viewModel;
        }

        ApplyResponsiveLayout(ActualWidth);
        if (viewModel is not null) await viewModel.ActivateAsync();
    }

    private void Page_Unloaded(object sender, RoutedEventArgs e) => viewModel?.Deactivate();
    private async void Retry_Click(object sender, RoutedEventArgs e) { if (viewModel is not null) await viewModel.LoadAsync(); }
    private void Page_SizeChanged(object sender, SizeChangedEventArgs e) => ApplyResponsiveLayout(e.NewSize.Width);
    private void Page_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        if (DashboardScrollViewer.ScrollableHeight <= 0) return;
        var targetOffset = Math.Clamp(DashboardScrollViewer.VerticalOffset - e.Delta, 0, DashboardScrollViewer.ScrollableHeight);
        if (Math.Abs(targetOffset - DashboardScrollViewer.VerticalOffset) < double.Epsilon) return;
        DashboardScrollViewer.ScrollToVerticalOffset(targetOffset);
        e.Handled = true;
    }

    private void OverviewAnchor_Click(object sender, RoutedEventArgs e) => ScrollTo(OverviewAnchor);
    private void DistributionAnchor_Click(object sender, RoutedEventArgs e) => ScrollTo(CompletedCard);
    private void ActivityAnchor_Click(object sender, RoutedEventArgs e) => ScrollTo(ActivityCard);
    private void RecentAnchor_Click(object sender, RoutedEventArgs e) => ScrollTo(RecentCompletedCard);

    private void ScrollTo(FrameworkElement target)
    {
        var offset = target.TranslatePoint(new System.Windows.Point(0, 0), DashboardContent).Y;
        DashboardScrollViewer.ScrollToVerticalOffset(Math.Clamp(offset, 0, DashboardScrollViewer.ScrollableHeight));
    }

    private void ApplyResponsiveLayout(double width)
    {
        var narrow = width < 720;
        LeftSectionColumn.Width = new GridLength(1, GridUnitType.Star);
        RightSectionColumn.Width = narrow ? new GridLength(0) : new GridLength(1, GridUnitType.Star);

        if (narrow)
        {
            SetPosition(ComparisonCard, 0, 0);
            SetPosition(CompletedCard, 1, 0);
            SetPosition(ActivityCard, 2, 0);
            SetPosition(FocusBreakdownCard, 3, 0);
            SetPosition(InsightsCard, 4, 0);
            SetPosition(FocusSummaryCard, 5, 0);
        }
        else
        {
            SetPosition(ComparisonCard, 0, 0);
            SetPosition(CompletedCard, 0, 1);
            SetPosition(ActivityCard, 1, 0);
            SetPosition(FocusBreakdownCard, 1, 1);
            SetPosition(InsightsCard, 2, 0);
            SetPosition(FocusSummaryCard, 2, 1);
        }
    }

    private static void SetPosition(UIElement element, int row, int column)
    {
        Grid.SetRow(element, row);
        Grid.SetColumn(element, column);
    }
}
