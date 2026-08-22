using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;
using Wpf.Ui.Controls;

namespace Quadrant.App.Views.Pages;

public partial class QuadrantsPage : Page
{
    private const string TaskIdFormat = "Quadrant.TaskId";
    private const string InboxTaskIdFormat = "Quadrant.InboxTaskId";
    private const double InboxBreakpoint = 900;
    private System.Windows.Point dragStartPoint;
    private Border? highlightedQuadrant;
    private MainViewModel? mainViewModel;
    private InboxPageViewModel? inboxViewModel;
    private long? pendingInboxDragTaskId;
    private bool? isNarrowInboxLayout;
    private bool isNarrowInboxExpanded;

    public QuadrantsPage()
    {
        InitializeComponent();
        AddHandler(UIElement.PreviewMouseLeftButtonDownEvent, new MouseButtonEventHandler(TaskCard_MouseLeftButtonDown));
        AddHandler(UIElement.PreviewMouseMoveEvent, new System.Windows.Input.MouseEventHandler(TaskCard_MouseMove));
        AddHandler(UIElement.PreviewKeyDownEvent, new System.Windows.Input.KeyEventHandler(TaskCard_PreviewKeyDown));
    }

    private async void Page_Loaded(object sender, RoutedEventArgs e)
    {
        if (inboxViewModel is null && DataContext is MainViewModel main)
        {
            mainViewModel = main;
            inboxViewModel = new InboxPageViewModel(main.TaskService, main.AppChangeHub);
            inboxViewModel.RecoverableError += InboxViewModel_RecoverableError;
            InboxPanel.DataContext = inboxViewModel;
            NarrowInboxButton.DataContext = inboxViewModel;
        }

        UpdateInboxLayout(ActualWidth);
        if (inboxViewModel is not null)
        {
            await inboxViewModel.ActivateAsync();
        }
    }

    private void Page_Unloaded(object sender, RoutedEventArgs e)
    {
        inboxViewModel?.Deactivate();
        ClearQuadrantFeedback();
        pendingInboxDragTaskId = null;
    }

    private async void Page_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
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
        else if (InboxList.IsKeyboardFocusWithin && InboxList.SelectedItem is TaskItem task && !IsInteractiveControl(e.OriginalSource as DependencyObject))
        {
            if (e.Key is >= Key.D1 and <= Key.D4 || e.Key is >= Key.NumPad1 and <= Key.NumPad4)
            {
                var quadrant = e.Key is >= Key.NumPad1 and <= Key.NumPad4 ? e.Key - Key.NumPad0 : e.Key - Key.D0;
                await AssignInboxTaskAsync(task, quadrant);
                e.Handled = true;
            }
            else if (e.Key == Key.Enter)
            {
                await EditInboxTaskAsync(task);
                e.Handled = true;
            }
            else if (e.Key == Key.Delete)
            {
                await ConfirmDeleteInboxTaskAsync(task);
                e.Handled = true;
            }
        }
    }

    private void Page_SizeChanged(object sender, SizeChangedEventArgs e) => UpdateInboxLayout(e.NewSize.Width);

    private void NarrowInboxButton_Click(object sender, RoutedEventArgs e)
    {
        isNarrowInboxExpanded = !isNarrowInboxExpanded;
        ApplyInboxLayout();
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

    private void InboxTask_PreviewMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        pendingInboxDragTaskId = sender is Border { DataContext: TaskItem task } && !IsInteractiveControl(e.OriginalSource as DependencyObject)
            ? task.Id
            : null;
        dragStartPoint = e.GetPosition(this);
    }

    private void InboxTask_PreviewMouseMove(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (e.LeftButton != MouseButtonState.Pressed || pendingInboxDragTaskId is not { } taskId || sender is not Border row)
        {
            if (e.LeftButton != MouseButtonState.Pressed)
            {
                pendingInboxDragTaskId = null;
            }

            return;
        }

        var current = e.GetPosition(this);
        if (Math.Abs(current.X - dragStartPoint.X) < SystemParameters.MinimumHorizontalDragDistance && Math.Abs(current.Y - dragStartPoint.Y) < SystemParameters.MinimumVerticalDragDistance)
        {
            return;
        }

        var data = new System.Windows.DataObject();
        data.SetData(TaskIdFormat, taskId);
        data.SetData(InboxTaskIdFormat, taskId);
        pendingInboxDragTaskId = null;
        System.Windows.DragDrop.DoDragDrop(row, data, System.Windows.DragDropEffects.Move);
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

        if (e.Data.GetDataPresent(InboxTaskIdFormat) && e.Data.GetData(InboxTaskIdFormat) is long inboxTaskId)
        {
            var task = RequireInboxViewModel().Tasks.FirstOrDefault(item => item.Id == inboxTaskId);
            if (task is not null)
            {
                await AssignInboxTaskAsync(task, targetQuadrantId);
            }
        }
        else
        {
            var viewModel = RequireMainViewModel();
            if (viewModel.MoveTaskCommand.CanExecute(new MoveTaskRequest(taskId, targetQuadrantId)))
            {
                await viewModel.MoveTaskCommand.ExecuteAsync(new MoveTaskRequest(taskId, targetQuadrantId));
            }
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

    private async Task AssignInboxTaskAsync(TaskItem task, int targetQuadrantId)
    {
        var moved = await RequireInboxViewModel().AssignQuadrantAsync(task, targetQuadrantId);
        if (moved is null)
        {
            return;
        }

        var main = RequireMainViewModel();
        await main.RefreshActiveTaskAsync(moved.Id);
        if (Window.GetWindow(this) is not MainWindow window)
        {
            return;
        }

        var targetName = main.Quadrants.FirstOrDefault(quadrant => quadrant.Id == targetQuadrantId)?.Name ?? $"Q{targetQuadrantId}";
        window.ShowUndoFeedback("任务已分类", $"已移至 {targetName}。", async () =>
        {
            var restored = await RequireInboxViewModel().RestoreToInboxAsync(moved.Id, targetQuadrantId);
            if (restored is null)
            {
                window.ShowFeedback("无法撤销", "任务状态已经发生变化。", ControlAppearance.Caution, SymbolRegular.Alert24);
                return;
            }

            await main.RefreshActiveTaskAsync(restored.Id);
            window.ShowFeedback("已撤销分类", "任务已恢复到 Inbox。", ControlAppearance.Success, SymbolRegular.ArrowUndo24);
        });
    }

    private async void InboxComplete_Click(object sender, RoutedEventArgs e)
    {
        if (((FrameworkElement)sender).Tag is TaskItem task)
        {
            await RequireInboxViewModel().CompleteAsync(task);
        }
    }

    private async void InboxPlanToday_Click(object sender, RoutedEventArgs e)
    {
        if (((FrameworkElement)sender).Tag is not TaskItem task)
        {
            return;
        }

        var updated = await RequireInboxViewModel().PlanForTodayAsync(task);
        if (updated is not null && Window.GetWindow(this) is MainWindow window)
        {
            window.ShowFeedback("已添加到 Today", "计划日期已设为今天。", ControlAppearance.Success, SymbolRegular.CalendarAdd24);
        }
    }

    private async void InboxEdit_Click(object sender, RoutedEventArgs e)
    {
        if (((FrameworkElement)sender).Tag is TaskItem task)
        {
            await EditInboxTaskAsync(task);
        }
    }

    private async Task EditInboxTaskAsync(TaskItem task)
    {
        var main = RequireMainViewModel();
        if (Window.GetWindow(this) is not MainWindow window)
        {
            return;
        }

        var editor = new TaskEditorWindow(
            new TaskEditorViewModel(
                main.Quadrants.Select(quadrant => new QuadrantDefinition(quadrant.Id, quadrant.Name, quadrant.Subtitle)),
                main.Clock,
                task,
                allowInbox: true))
        {
            Owner = window
        };
        if (editor.ShowDialog() == true && editor.UpdateResult is { } update)
        {
            await main.UpdateAsync(update);
            window.ShowFeedback("任务已更新", update.Title);
        }
    }

    private async void InboxDelete_Click(object sender, RoutedEventArgs e)
    {
        if (((FrameworkElement)sender).Tag is TaskItem task)
        {
            await ConfirmDeleteInboxTaskAsync(task);
        }
    }

    private async Task ConfirmDeleteInboxTaskAsync(TaskItem task)
    {
        if (Window.GetWindow(this) is not MainWindow window)
        {
            return;
        }

        var result = await window.ShowDialogAsync("删除任务？", "此操作会永久删除该任务。", "删除", ControlAppearance.Danger);
        if (result == ContentDialogResult.Primary)
        {
            await RequireInboxViewModel().DeleteAsync(task);
        }
    }

    private async void InboxRetry_Click(object sender, RoutedEventArgs e) => await RequireInboxViewModel().LoadAsync();

    private void InboxViewModel_RecoverableError(object? sender, RecoverableOperationErrorEventArgs e)
    {
        if (Window.GetWindow(this) is MainWindow window)
        {
            window.ShowFeedback(e.Title, e.Exception.Message, ControlAppearance.Caution, SymbolRegular.Alert24);
        }
    }

    private void UpdateInboxLayout(double width)
    {
        var narrow = width < InboxBreakpoint;
        if (isNarrowInboxLayout == narrow)
        {
            return;
        }

        isNarrowInboxLayout = narrow;
        isNarrowInboxExpanded = false;
        ApplyInboxLayout();
    }

    private void ApplyInboxLayout()
    {
        if (isNarrowInboxLayout == true)
        {
            InboxColumn.Width = new GridLength(0);
            InboxGapColumn.Width = new GridLength(0);
            Grid.SetRow(InboxPanel, 0);
            Grid.SetColumn(InboxPanel, 0);
            Grid.SetColumnSpan(InboxPanel, 3);
            InboxPanel.Margin = new Thickness(0, 0, 0, 16);
            InboxPanel.MaxHeight = 180;
            InboxPanel.Visibility = isNarrowInboxExpanded ? Visibility.Visible : Visibility.Collapsed;
            Grid.SetRow(MatrixPanel, 1);
            Grid.SetColumn(MatrixPanel, 0);
            Grid.SetColumnSpan(MatrixPanel, 3);
            NarrowInboxButton.Visibility = Visibility.Visible;
            NarrowInboxButtonLabel.Text = isNarrowInboxExpanded ? "收起 Inbox" : "Inbox";
            return;
        }

        InboxColumn.Width = new GridLength(248);
        InboxGapColumn.Width = new GridLength(16);
        Grid.SetRow(InboxPanel, 1);
        Grid.SetColumn(InboxPanel, 0);
        Grid.SetColumnSpan(InboxPanel, 1);
        InboxPanel.Margin = new Thickness(0);
        InboxPanel.MaxHeight = double.PositiveInfinity;
        InboxPanel.Visibility = Visibility.Visible;
        Grid.SetRow(MatrixPanel, 1);
        Grid.SetColumn(MatrixPanel, 2);
        Grid.SetColumnSpan(MatrixPanel, 1);
        NarrowInboxButton.Visibility = Visibility.Collapsed;
        NarrowInboxButtonLabel.Text = "Inbox";
    }

    private MainViewModel RequireMainViewModel() => mainViewModel ?? DataContext as MainViewModel ?? throw new InvalidOperationException("Home is not initialized.");

    private InboxPageViewModel RequireInboxViewModel() => inboxViewModel ?? throw new InvalidOperationException("Home Inbox is not initialized.");

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
