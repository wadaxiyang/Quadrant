using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using Quadrant.App.ViewModels;

namespace Quadrant.App.Views.Pages;

public partial class QuadrantsPage : Page
{
    private const string TaskIdFormat = "Quadrant.TaskId";
    private System.Windows.Point dragStartPoint;
    private Border? highlightedQuadrant;

    public QuadrantsPage()
    {
        InitializeComponent();
        AddHandler(UIElement.PreviewMouseLeftButtonDownEvent, new MouseButtonEventHandler(TaskCard_MouseLeftButtonDown));
        AddHandler(UIElement.PreviewMouseMoveEvent, new System.Windows.Input.MouseEventHandler(TaskCard_MouseMove));
        AddHandler(UIElement.PreviewKeyDownEvent, new System.Windows.Input.KeyEventHandler(TaskCard_PreviewKeyDown));
    }

    private void Page_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (e.Key == Key.F && Keyboard.Modifiers == ModifierKeys.Control)
        {
            SearchBox.Focus();
            SearchBox.SelectAll();
            e.Handled = true;
        }
        else if (e.Key == Key.Escape && SearchBox.IsKeyboardFocusWithin)
        {
            SearchBox.Clear();
            ((MainViewModel)DataContext).SelectedFilter = Quadrant.Core.Enums.TaskFilter.All;
            Keyboard.ClearFocus();
            e.Handled = true;
        }
    }

    private void TaskCard_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (e.Key is not (Key.Enter or Key.Space) || IsInteractiveControl(e.OriginalSource as DependencyObject) || FindTaskCard(e.OriginalSource as DependencyObject)?.DataContext is not TaskCardViewModel task)
        {
            return;
        }

        if (task.CompleteCommand.CanExecute(task.Id))
        {
            task.CompleteCommand.Execute(task.Id);
            e.Handled = true;
        }
    }

    private static bool IsInteractiveControl(DependencyObject? source)
    {
        while (source is not null)
        {
            if (source is System.Windows.Controls.Primitives.ButtonBase or System.Windows.Controls.MenuItem or System.Windows.Controls.Menu)
            {
                return true;
            }

            if (source is Border border && border.DataContext is TaskCardViewModel)
            {
                return false;
            }

            source = source is Visual ? VisualTreeHelper.GetParent(source) : null;
        }

        return false;
    }

    private void TaskCard_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (FindTaskCard(e.OriginalSource as DependencyObject) is not null)
        {
            dragStartPoint = e.GetPosition(this);
        }
    }

    private void TaskCard_MouseMove(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (e.LeftButton != MouseButtonState.Pressed || FindTaskCard(e.OriginalSource as DependencyObject) is not Border card)
        {
            return;
        }

        var current = e.GetPosition(this);
        if (Math.Abs(current.X - dragStartPoint.X) < SystemParameters.MinimumHorizontalDragDistance && Math.Abs(current.Y - dragStartPoint.Y) < SystemParameters.MinimumVerticalDragDistance)
        {
            return;
        }

        if (card.DataContext is not TaskCardViewModel task)
        {
            return;
        }

        var data = new System.Windows.DataObject(TaskIdFormat, task.Id);
        System.Windows.DragDrop.DoDragDrop(card, data, System.Windows.DragDropEffects.Move);
        ClearQuadrantFeedback();
    }

    private void Quadrant_DragOver(object sender, System.Windows.DragEventArgs e)
    {
        if (!e.Data.GetDataPresent(TaskIdFormat) || sender is not Border target)
        {
            e.Effects = System.Windows.DragDropEffects.None;
            e.Handled = true;
            return;
        }

        e.Effects = System.Windows.DragDropEffects.Move;
        SetQuadrantFeedback(target);
        e.Handled = true;
    }

    private async void Quadrant_Drop(object sender, System.Windows.DragEventArgs e)
    {
        ClearQuadrantFeedback();
        if (sender is not Border target || !e.Data.GetDataPresent(TaskIdFormat) || e.Data.GetData(TaskIdFormat) is not long taskId || target.Tag is not string targetText || !int.TryParse(targetText, out var targetQuadrantId))
        {
            return;
        }

        var viewModel = (MainViewModel)DataContext;
        if (viewModel.MoveTaskCommand.CanExecute(new MoveTaskRequest(taskId, targetQuadrantId)))
        {
            await viewModel.MoveTaskCommand.ExecuteAsync(new MoveTaskRequest(taskId, targetQuadrantId));
        }

        e.Handled = true;
    }

    private void Quadrant_DragLeave(object sender, System.Windows.DragEventArgs e)
    {
        if (sender is Border target)
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
        target.SetResourceReference(Border.BorderBrushProperty, "SystemAccentColorPrimaryBrush");
    }

    private void ClearQuadrantFeedback(Border? target = null)
    {
        var border = target ?? highlightedQuadrant;
        if (border is null)
        {
            return;
        }

        border.BorderThickness = new Thickness(1);
        border.SetResourceReference(Border.BorderBrushProperty, "CardStrokeColorDefaultBrush");
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
