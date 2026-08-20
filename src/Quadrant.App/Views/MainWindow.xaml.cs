using System.Windows;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;

namespace Quadrant.App.Views;

public partial class MainWindow : System.Windows.Window
{
    public MainWindow()
    {
        InitializeComponent();
        Loaded += MainWindow_Loaded;
    }

    private void MainWindow_Loaded(object sender, RoutedEventArgs e)
    {
        var viewModel = (MainViewModel)DataContext;
        viewModel.NewTaskRequested += NewTaskRequested;
        viewModel.EditTaskRequested += EditTaskRequested;
        viewModel.DeleteTaskRequested += DeleteTaskRequested;
    }

    private async void NewTaskRequested(object? sender, EventArgs e)
    {
        var viewModel = (MainViewModel)DataContext;
        var editor = new TaskEditorWindow(new TaskEditorViewModel(viewModel.Quadrants.Select(ToDefinition)));
        editor.Owner = this;
        if (editor.ShowDialog() == true && editor.DraftResult is { } draft)
        {
            await viewModel.CreateAsync(draft);
        }
    }

    private async void EditTaskRequested(object? sender, TaskItem task)
    {
        var viewModel = (MainViewModel)DataContext;
        var editor = new TaskEditorWindow(new TaskEditorViewModel(viewModel.Quadrants.Select(ToDefinition), task));
        editor.Owner = this;
        if (editor.ShowDialog() == true && editor.UpdateResult is { } update)
        {
            await viewModel.UpdateAsync(update);
        }
    }

    private async void DeleteTaskRequested(object? sender, long id)
    {
        if (MessageBox.Show("确定删除此任务吗？", "删除任务", MessageBoxButton.OKCancel, MessageBoxImage.Warning) != MessageBoxResult.OK)
        {
            return;
        }

        await ((MainViewModel)DataContext).ConfirmedDeleteAsync(id);
    }

    private static QuadrantDefinition ToDefinition(QuadrantViewModel quadrant) =>
        new(quadrant.Id, quadrant.Name, quadrant.Subtitle);
}
