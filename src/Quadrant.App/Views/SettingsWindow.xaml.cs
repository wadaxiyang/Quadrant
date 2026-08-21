using System.Windows;
using Quadrant.App.ViewModels;

namespace Quadrant.App.Views;

public partial class SettingsWindow : Window
{
    public SettingsWindow(SettingsViewModel viewModel) { InitializeComponent(); DataContext = viewModel; }
    public SettingsViewModel Settings => (SettingsViewModel)DataContext;
    private async void Save_Click(object sender, RoutedEventArgs e)
    {
        try { await Settings.SaveAsync(); DialogResult = true; }
        catch (Exception exception) { System.Windows.MessageBox.Show(exception.Message, "设置保存失败", System.Windows.MessageBoxButton.OK, System.Windows.MessageBoxImage.Warning); }
    }
}
