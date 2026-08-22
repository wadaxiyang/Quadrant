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

    private void ApplyResponsiveLayout(double width)
    {
        var narrow = width < 720;
        var kpiPanelKey = width < 1000 ? "ReviewPrimaryKpiNarrowPanel" : "ReviewPrimaryKpiWidePanel";
        var kpiPanel = (ItemsPanelTemplate)Resources[kpiPanelKey];
        if (!ReferenceEquals(PrimaryKpiItems.ItemsPanel, kpiPanel)) PrimaryKpiItems.ItemsPanel = kpiPanel;

        LeftSectionColumn.Width = new GridLength(1, GridUnitType.Star);
        SectionGapColumn.Width = narrow ? new GridLength(0) : new GridLength(16);
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
            SetPosition(CompletedCard, 0, 2);
            SetPosition(ActivityCard, 1, 0);
            SetPosition(FocusBreakdownCard, 1, 2);
            SetPosition(InsightsCard, 2, 0);
            SetPosition(FocusSummaryCard, 2, 2);
        }
    }

    private static void SetPosition(UIElement element, int row, int column)
    {
        Grid.SetRow(element, row);
        Grid.SetColumn(element, column);
    }
}
