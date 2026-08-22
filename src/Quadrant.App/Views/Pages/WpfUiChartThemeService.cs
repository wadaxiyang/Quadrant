using ScottPlot;
using Wpf.Ui.Appearance;

namespace Quadrant.App.Views.Pages;

internal sealed class WpfUiChartThemeService : IChartThemeService
{
    public void Apply(Plot plot)
    {
        var isDark = ApplicationThemeManager.GetAppTheme() is ApplicationTheme.Dark;
        var background = ScottPlot.Color.FromHex(isDark ? "#202020" : "#FFFFFF");
        var foreground = ScottPlot.Color.FromHex(isDark ? "#FFFFFF" : "#1A1A1A");
        var grid = ScottPlot.Color.FromHex(isDark ? "#3A3A3A" : "#E0E0E0");
        plot.FigureBackground.Color = background;
        plot.DataBackground.Color = background;
        plot.Axes.Color(foreground);
        plot.Grid.MajorLineColor = grid;
    }
}
