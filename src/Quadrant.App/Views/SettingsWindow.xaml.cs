using System.Windows;
using Quadrant.App.ViewModels;

namespace Quadrant.App.Views;

public partial class SettingsWindow : Wpf.Ui.Controls.FluentWindow
{
    public SettingsWindow(SettingsViewModel viewModel) { InitializeComponent(); DataContext = viewModel; }
    public SettingsViewModel Settings => (SettingsViewModel)DataContext;
    public Quadrant.Core.Models.AppSettings? DesiredSettings { get; private set; }
    public IReadOnlyList<Quadrant.Core.Models.QuadrantDefinition>? DesiredQuadrants { get; private set; }

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            DesiredSettings = Settings.BuildSettings();
            DesiredQuadrants = Settings.BuildQuadrants();
            DialogResult = true;
        }
        catch (Exception exception)
        {
            ValidationInfo.Message = exception.Message;
            ValidationInfo.IsOpen = true;
        }
    }
}
