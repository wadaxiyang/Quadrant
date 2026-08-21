using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Interop;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;

namespace Quadrant.App.Views;

public partial class MainWindow : System.Windows.Window
{
    private const string TaskIdFormat = "Quadrant.TaskId";
    private System.Windows.Point dragStartPoint;
    private Border? highlightedQuadrant;
    private Quadrant.Infrastructure.Windows.GlobalHotkeyService? globalHotkeyService;
    private HwndSource? windowSource;
    private bool quickAddOpen;

    public MainWindow()
    {
        InitializeComponent();
        Loaded += MainWindow_Loaded;
        SourceInitialized += MainWindow_SourceInitialized;
        Closed += MainWindow_Closed;
        Closing += MainWindow_Closing;
        AddHandler(UIElement.PreviewMouseLeftButtonDownEvent, new MouseButtonEventHandler(TaskCard_MouseLeftButtonDown));
        AddHandler(UIElement.PreviewMouseMoveEvent, new System.Windows.Input.MouseEventHandler(TaskCard_MouseMove));
        AddHandler(UIElement.PreviewKeyDownEvent, new System.Windows.Input.KeyEventHandler(TaskCard_PreviewKeyDown));
    }

    public event EventHandler? GlobalHotkeyPressed;

    public bool IsCloseToTray { get; set; } = true;
    public event EventHandler? SettingsRequested;

    public void ConfigureGlobalHotkey(Quadrant.Infrastructure.Windows.GlobalHotkeyService service)
    {
        globalHotkeyService = service;
    }

    private void MainWindow_SourceInitialized(object? sender, EventArgs e)
    {
        if (globalHotkeyService is null)
        {
            return;
        }

        windowSource = (HwndSource)PresentationSource.FromVisual(this)!;
        windowSource.AddHook(WindowSourceHook);
        globalHotkeyService.Register(new WindowInteropHelper(this).Handle);
    }

    private void MainWindow_Closed(object? sender, EventArgs e)
    {
        if (windowSource is not null)
        {
            windowSource.RemoveHook(WindowSourceHook);
        }

        globalHotkeyService?.Unregister(new WindowInteropHelper(this).Handle);
    }

    private void MainWindow_Closing(object? sender, System.ComponentModel.CancelEventArgs e)
    {
        if (IsCloseToTray)
        {
            e.Cancel = true;
            Hide();
        }
    }

    public void ShowFromTray()
    {
        Show();
        if (WindowState == WindowState.Minimized)
        {
            WindowState = WindowState.Normal;
        }

        Activate();
        Focus();
    }

    private IntPtr WindowSourceHook(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        if (globalHotkeyService?.IsHotkeyMessage(msg, wParam) == true)
        {
            GlobalHotkeyPressed?.Invoke(this, EventArgs.Empty);
            handled = true;
        }

        return IntPtr.Zero;
    }

    private void MainWindow_Loaded(object sender, RoutedEventArgs e)
    {
        var viewModel = (MainViewModel)DataContext;
        viewModel.NewTaskRequested += NewTaskRequested;
        viewModel.EditTaskRequested += EditTaskRequested;
        viewModel.DeleteTaskRequested += DeleteTaskRequested;
    }

    private async void Completed_Click(object sender, RoutedEventArgs e)
    {
        var viewModel = (MainViewModel)DataContext;
        await viewModel.LoadCompletedAsync();
        var window = new CompletedWindow(viewModel)
        {
            Owner = this
        };
        window.ShowDialog();
    }

    private void Settings_Click(object sender, RoutedEventArgs e) => SettingsRequested?.Invoke(this, EventArgs.Empty);

    private void MainWindow_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
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
        if (e.Key is not (Key.Enter or Key.Space) || e.OriginalSource is System.Windows.Controls.Button || FindTaskCard(e.OriginalSource as DependencyObject)?.DataContext is not TaskCardViewModel task)
        {
            return;
        }

        if (task.CompleteCommand.CanExecute(task.Id))
        {
            task.CompleteCommand.Execute(task.Id);
            e.Handled = true;
        }
    }

    private async void NewTaskRequested(object? sender, EventArgs e)
    {
        try
        {
            var viewModel = (MainViewModel)DataContext;
            var editor = new TaskEditorWindow(new TaskEditorViewModel(viewModel.Quadrants.Select(ToDefinition), viewModel.Clock));
            editor.Owner = this;
            if (editor.ShowDialog() == true && editor.DraftResult is { } draft)
            {
                await viewModel.CreateAsync(draft);
            }
        }
        catch (Exception exception)
        {
            ShowRecoverableError("任务保存失败", exception);
        }
    }

    private async void EditTaskRequested(object? sender, TaskItem task)
    {
        try
        {
            var viewModel = (MainViewModel)DataContext;
            var editor = new TaskEditorWindow(new TaskEditorViewModel(viewModel.Quadrants.Select(ToDefinition), viewModel.Clock, task));
            editor.Owner = this;
            if (editor.ShowDialog() == true && editor.UpdateResult is { } update)
            {
                await viewModel.UpdateAsync(update);
            }
        }
        catch (Exception exception)
        {
            ShowRecoverableError("任务保存失败", exception);
        }
    }

    private void ShowRecoverableError(string title, Exception exception)
    {
        System.Windows.MessageBox.Show($"{title}。\n{exception.Message}", title, System.Windows.MessageBoxButton.OK, System.Windows.MessageBoxImage.Warning);
    }

    public async Task ActivateAndOpenTaskAsync(long id)
    {
        Activate();
        if (WindowState == WindowState.Minimized)
        {
            WindowState = WindowState.Normal;
        }

        await ((MainViewModel)DataContext).OpenTaskAsync(id);
    }

    public async Task ShowQuickAddAsync()
    {
        if (quickAddOpen)
        {
            return;
        }

        quickAddOpen = true;
        var viewModel = (MainViewModel)DataContext;
        try
        {
            var editor = new QuickAddWindow(new TaskEditorViewModel(viewModel.Quadrants.Select(ToDefinition), viewModel.Clock))
            {
                Owner = IsVisible ? this : null
            };

            if (editor.ShowDialog() == true && editor.DraftResult is { } draft)
            {
                await viewModel.CreateAsync(draft);
            }
        }
        finally
        {
            quickAddOpen = false;
        }
    }

    private async void DeleteTaskRequested(object? sender, long id)
    {
        if (System.Windows.MessageBox.Show("确定删除此任务吗？", "删除任务", System.Windows.MessageBoxButton.OKCancel, System.Windows.MessageBoxImage.Warning) != System.Windows.MessageBoxResult.OK)
        {
            return;
        }

        try
        {
            await ((MainViewModel)DataContext).ConfirmedDeleteAsync(id);
        }
        catch (Exception exception)
        {
            ShowRecoverableError("任务删除失败", exception);
        }
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

    private void TaskCard_MouseMove(object sender, System.Windows.Input.MouseEventArgs e)
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
            System.Windows.MessageBox.Show("任务移动失败，原位置未改变。", "移动任务", System.Windows.MessageBoxButton.OK, System.Windows.MessageBoxImage.Information);
        }
        e.Handled = true;
    }

    private void Quadrant_DragLeave(object sender, System.Windows.DragEventArgs e)
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
