using System.Windows;
using System.Windows.Controls;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;
using Wpf.Ui.Controls;

namespace Quadrant.App.Views.Pages;

public partial class TodayPage : Page
{
    private MainViewModel? main; private TodayPageViewModel? viewModel;
    public TodayPage() => InitializeComponent();
    private async void Page_Loaded(object sender, RoutedEventArgs e) { if (viewModel is null && DataContext is MainViewModel value) { main = value; viewModel = new TodayPageViewModel(value.TodayQueryService, value.AppChangeHub); DataContext = viewModel; } if (viewModel is not null) await viewModel.ActivateAsync(); }
    private void Page_Unloaded(object sender, RoutedEventArgs e) => viewModel?.Deactivate();
    private async void Retry_Click(object sender, RoutedEventArgs e) { if (viewModel is not null) await viewModel.LoadAsync(); }
    private async void Complete_Click(object sender, RoutedEventArgs e) => await MutateAsync((TaskItem)((FrameworkElement)sender).Tag, task => main!.TaskService.SetCompletedAsync(task.Id, true));
    private async void Today_Click(object sender, RoutedEventArgs e) => await MutateAsync((TaskItem)((FrameworkElement)sender).Tag, task => main!.TaskService.PlanForTodayAsync(task.Id));
    private async void Tomorrow_Click(object sender, RoutedEventArgs e) => await MutateAsync((TaskItem)((FrameworkElement)sender).Tag, task => main!.TaskService.PlanForDateAsync(task.Id, main!.Clock.LocalDate.AddDays(1)));
    private async void Remove_Click(object sender, RoutedEventArgs e) => await MutateAsync((TaskItem)((FrameworkElement)sender).Tag, task => main!.TaskService.RemovePlanAsync(task.Id));
    private async Task MutateAsync(TaskItem task, Func<TaskItem, Task<TaskItem>> mutation) { try { await mutation(task); await main!.RefreshActiveTaskAsync(task.Id); await viewModel!.LoadAsync(); } catch (Exception exception) { if (Window.GetWindow(this) is MainWindow window) window.ShowFeedback("Today 操作失败", exception.Message, ControlAppearance.Caution, SymbolRegular.Alert24); } }
}
