using System.Windows;
using Quadrant.App.ViewModels;

namespace Quadrant.App.Views;

public partial class SettingsWindow : Window
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
        catch (Exception exception) { System.Windows.MessageBox.Show(exception.Message, "设置保存失败", System.Windows.MessageBoxButton.OK, System.Windows.MessageBoxImage.Warning); }
    }
}
