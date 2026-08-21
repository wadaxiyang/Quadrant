using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;
using Wpf.Ui.Controls;

namespace Quadrant.App.Views.Pages;

public partial class InboxPage : Page
{
    private MainViewModel? mainViewModel;
    private InboxPageViewModel? viewModel;

    public InboxPage() => InitializeComponent();

    private async void Page_Loaded(object sender, RoutedEventArgs e)
    {
        if (viewModel is null && DataContext is MainViewModel main)
        {
            mainViewModel = main;
            viewModel = new InboxPageViewModel(main.TaskService, main.AppChangeHub);
            viewModel.RecoverableError += ViewModel_RecoverableError;
            DataContext = viewModel;
        }

        if (viewModel is not null)
        {
            await viewModel.ActivateAsync();
        }
    }

    private void Page_Unloaded(object sender, RoutedEventArgs e)
    {
        if (viewModel is not null)
        {
            viewModel.Deactivate();
        }
    }

    private async void Retry_Click(object sender, RoutedEventArgs e) => await RequireViewModel().LoadAsync();
    private async void Complete_Click(object sender, RoutedEventArgs e) => await RequireViewModel().CompleteAsync((TaskItem)((FrameworkElement)sender).Tag);
    private async void ClassifyQ1_Click(object sender, RoutedEventArgs e) => await AssignAsync(sender, 1);
    private async void ClassifyQ2_Click(object sender, RoutedEventArgs e) => await AssignAsync(sender, 2);
    private async void ClassifyQ3_Click(object sender, RoutedEventArgs e) => await AssignAsync(sender, 3);
    private async void ClassifyQ4_Click(object sender, RoutedEventArgs e) => await AssignAsync(sender, 4);

    private async Task AssignAsync(object sender, int quadrantId)
    {
        await RequireViewModel().AssignQuadrantAsync((TaskItem)((FrameworkElement)sender).Tag, quadrantId);
        if (Window.GetWindow(this) is MainWindow window)
        {
            window.ShowFeedback("任务已分类", $"已移至 Q{quadrantId}。", ControlAppearance.Success, SymbolRegular.ArrowRight24);
        }
    }

    private async void Edit_Click(object sender, RoutedEventArgs e)
    {
        await EditAsync((TaskItem)((FrameworkElement)sender).Tag);
    }

    private async Task EditAsync(TaskItem task)
    {
        if (mainViewModel is null || Window.GetWindow(this) is not MainWindow window) return;
        var editor = new TaskEditorWindow(new TaskEditorViewModel(mainViewModel.Quadrants.Select(q => new QuadrantDefinition(q.Id, q.Name, q.Subtitle)), mainViewModel.Clock, task, allowInbox: true)) { Owner = window };
        if (editor.ShowDialog() == true && editor.UpdateResult is { } update)
        {
            await mainViewModel.UpdateAsync(update);
            window.ShowFeedback("任务已更新", update.Title);
        }
    }

    private async void Delete_Click(object sender, RoutedEventArgs e)
    {
        await ConfirmDeleteAsync((TaskItem)((FrameworkElement)sender).Tag);
    }

    private async Task ConfirmDeleteAsync(TaskItem task)
    {
        if (Window.GetWindow(this) is not MainWindow window) return;
        var result = await window.ShowDialogAsync("删除任务？", "此操作会永久删除该任务。", "删除", ControlAppearance.Danger);
        if (result == ContentDialogResult.Primary) await RequireViewModel().DeleteAsync(task);
    }

    private async void Page_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (InboxList.SelectedItem is not TaskItem task || IsEditingInput(e.OriginalSource as DependencyObject)) return;
        if (e.Key is >= Key.D1 and <= Key.D4 || e.Key is >= Key.NumPad1 and <= Key.NumPad4)
        {
            var quadrant = e.Key is >= Key.NumPad1 and <= Key.NumPad4 ? e.Key - Key.NumPad0 : e.Key - Key.D0;
            await RequireViewModel().AssignQuadrantAsync(task, quadrant);
            e.Handled = true;
        }
        else if (e.Key == Key.Enter)
        {
            await EditAsync(task);
            e.Handled = true;
        }
        else if (e.Key == Key.Delete)
        {
            await ConfirmDeleteAsync(task);
            e.Handled = true;
        }
    }

    private void ViewModel_RecoverableError(object? sender, RecoverableOperationErrorEventArgs e)
    {
        if (Window.GetWindow(this) is MainWindow window) window.ShowFeedback(e.Title, e.Exception.Message, ControlAppearance.Caution, SymbolRegular.Alert24);
    }

    private InboxPageViewModel RequireViewModel() => viewModel ?? throw new InvalidOperationException("Inbox is not initialized.");

    private static bool IsEditingInput(DependencyObject? source)
    {
        while (source is not null)
        {
            if (source is System.Windows.Controls.TextBox or System.Windows.Controls.ComboBox or Wpf.Ui.Controls.Button) return true;
            source = source is Visual visual ? VisualTreeHelper.GetParent(visual) : null;
        }

        return false;
    }
}
