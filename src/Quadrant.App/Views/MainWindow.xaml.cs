using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;

namespace Quadrant.App.Views;

public partial class MainWindow : System.Windows.Window
{
    private const string TaskIdFormat = "Quadrant.TaskId";
    private Point dragStartPoint;
    private Border? highlightedQuadrant;

    public MainWindow()
    {
        InitializeComponent();
        Loaded += MainWindow_Loaded;
        AddHandler(UIElement.PreviewMouseLeftButtonDownEvent, new MouseButtonEventHandler(TaskCard_MouseLeftButtonDown));
        AddHandler(UIElement.PreviewMouseMoveEvent, new MouseEventHandler(TaskCard_MouseMove));
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

    private void TaskCard_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (FindTaskCard(e.OriginalSource as DependencyObject) is not null)
        {
            dragStartPoint = e.GetPosition(this);
        }
    }

    private void TaskCard_MouseMove(object sender, MouseEventArgs e)
    {
        if (e.LeftButton != MouseButtonState.Pressed || FindTaskCard(e.OriginalSource as DependencyObject) is not Border card)
        {
            return;
        }

        var current = e.GetPosition(this);
        if (Math.Abs(current.X - dragStartPoint.X) < SystemParameters.MinimumHorizontalDragDistance &&
            Math.Abs(current.Y - dragStartPoint.Y) < SystemParameters.MinimumVerticalDragDistance)
        {
            return;
        }

        if (card.DataContext is not TaskCardViewModel task)
        {
            return;
        }

        var data = new DataObject(TaskIdFormat, task.Id);
        DragDrop.DoDragDrop(card, data, DragDropEffects.Move);
        ClearQuadrantFeedback();
    }

    private void Quadrant_DragOver(object sender, DragEventArgs e)
    {
        if (!e.Data.GetDataPresent(TaskIdFormat) || sender is not Border target)
        {
            e.Effects = DragDropEffects.None;
            e.Handled = true;
            return;
        }

        e.Effects = DragDropEffects.Move;
        SetQuadrantFeedback(target);
        e.Handled = true;
    }

    private async void Quadrant_Drop(object sender, DragEventArgs e)
    {
        ClearQuadrantFeedback();
        if (sender is not Border target || !e.Data.GetDataPresent(TaskIdFormat) || e.Data.GetData(TaskIdFormat) is not long taskId || target.Tag is not string targetText || !int.TryParse(targetText, out var targetQuadrantId))
        {
            return;
        }

        try
        {
            var viewModel = (MainViewModel)DataContext;
            if (viewModel.MoveTaskCommand.CanExecute(new MoveTaskRequest(taskId, targetQuadrantId)))
            {
                await viewModel.MoveTaskCommand.ExecuteAsync(new MoveTaskRequest(taskId, targetQuadrantId));
            }
        }
        catch (Exception)
        {
            MessageBox.Show("任务移动失败，原位置未改变。", "移动任务", MessageBoxButton.OK, MessageBoxImage.Information);
        }
        e.Handled = true;
    }

    private void Quadrant_DragLeave(object sender, DragEventArgs e)
    {
        if (e.OriginalSource is Border target)
        {
            ClearQuadrantFeedback(target);
        }
    }

    private void SetQuadrantFeedback(Border target)
    {
        if (highlightedQuadrant == target)
        {
            return;
        }

        ClearQuadrantFeedback();
        highlightedQuadrant = target;
        target.BorderThickness = new Thickness(2);
        target.SetResourceReference(Border.BorderBrushProperty, "SystemControlHighlightAltAccentBrush");
    }

    private void ClearQuadrantFeedback(Border? target = null)
    {
        var border = target ?? highlightedQuadrant;
        if (border is null)
        {
            return;
        }

        border.BorderThickness = new Thickness(1);
        border.SetResourceReference(Border.BorderBrushProperty, "ControlStrokeColorDefaultBrush");
        if (highlightedQuadrant == border)
        {
            highlightedQuadrant = null;
        }
    }

    private static Border? FindTaskCard(DependencyObject? source)
    {
        while (source is not null)
        {
            if (source is Border border && border.DataContext is TaskCardViewModel)
            {
                return border;
            }

            source = source is Visual ? VisualTreeHelper.GetParent(source) : null;
        }

        return null;
    }
}
