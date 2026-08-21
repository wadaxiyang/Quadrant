using System.Windows;
using System.Windows.Interop;
using Quadrant.App.ViewModels;
using Quadrant.App.Views.Pages;
using Quadrant.Core.Models;
using Wpf.Ui;
using Wpf.Ui.Controls;

namespace Quadrant.App.Views;

public partial class MainWindow : FluentWindow
{
    private readonly SnackbarService snackbarService = new();
    private Quadrant.Infrastructure.Windows.GlobalHotkeyService? globalHotkeyService;
    private HwndSource? windowSource;
    private bool quickAddOpen;
    private bool isApplicationExiting;
    private bool viewModelHandlersAttached;
    private bool initialNavigationCompleted;
    private bool dialogOpen;

    public MainWindow()
    {
        InitializeComponent();
        snackbarService.SetSnackbarPresenter(SnackbarPresenter);
        Loaded += MainWindow_Loaded;
        SourceInitialized += MainWindow_SourceInitialized;
        Closed += MainWindow_Closed;
        Closing += MainWindow_Closing;
    }

    public event EventHandler? GlobalHotkeyPressed;
    public event EventHandler? ExitRequested;
    public event EventHandler? SettingsRequested;

    public bool IsCloseToTray { get; set; } = true;

    public void ConfigureGlobalHotkey(Quadrant.Infrastructure.Windows.GlobalHotkeyService service) =>
        globalHotkeyService = service;

    private void MainWindow_SourceInitialized(object? sender, EventArgs e)
    {
        if (globalHotkeyService is null)
        {
            return;
        }

        var handle = new WindowInteropHelper(this).Handle;
        windowSource = HwndSource.FromHwnd(handle)
            ?? throw new InvalidOperationException("无法获取主窗口的 HWND 消息源。");
        windowSource.AddHook(WindowSourceHook);
        globalHotkeyService.Register(handle);
    }

    private void MainWindow_Closed(object? sender, EventArgs e)
    {
        if (windowSource is not null)
        {
            windowSource.RemoveHook(WindowSourceHook);
        }

        globalHotkeyService?.Unregister(new WindowInteropHelper(this).Handle);
        if (viewModelHandlersAttached && DataContext is MainViewModel viewModel)
        {
            viewModel.NewTaskRequested -= NewTaskRequested;
            viewModel.EditTaskRequested -= EditTaskRequested;
            viewModel.DeleteTaskRequested -= DeleteTaskRequested;
            viewModel.RecoverableError -= ViewModel_RecoverableError;
            viewModelHandlersAttached = false;
        }
    }

    private void MainWindow_Closing(object? sender, System.ComponentModel.CancelEventArgs e)
    {
        if (isApplicationExiting)
        {
            return;
        }

        if (IsCloseToTray)
        {
            e.Cancel = true;
            Hide();
            return;
        }

        e.Cancel = true;
        ExitRequested?.Invoke(this, EventArgs.Empty);
    }

    public void AllowApplicationExit() => isApplicationExiting = true;

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

    public void ShowFeedback(
        string title,
        string message,
        ControlAppearance appearance = ControlAppearance.Success,
        SymbolRegular symbol = SymbolRegular.CheckmarkCircle24)
    {
        SnackbarPresenter.HideCurrent();
        snackbarService.Show(title, message, appearance, new SymbolIcon(symbol), TimeSpan.FromSeconds(4));
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
        if (!viewModelHandlersAttached)
        {
            viewModel.NewTaskRequested += NewTaskRequested;
            viewModel.EditTaskRequested += EditTaskRequested;
            viewModel.DeleteTaskRequested += DeleteTaskRequested;
            viewModel.RecoverableError += ViewModel_RecoverableError;
            viewModelHandlersAttached = true;
        }

        if (!initialNavigationCompleted)
        {
            RootNavigationView.Navigate(typeof(QuadrantsPage), viewModel);
            initialNavigationCompleted = true;
        }
    }

    private void RootNavigationView_Navigated(NavigationView sender, NavigatedEventArgs args)
    {
        if (args.Page is FrameworkElement page && page.DataContext is null)
        {
            page.DataContext = DataContext;
        }
    }

    private async void Completed_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var viewModel = (MainViewModel)DataContext;
            await viewModel.LoadCompletedAsync();
            new CompletedWindow(viewModel) { Owner = this }.ShowDialog();
        }
        catch (Exception exception)
        {
            await ShowRecoverableErrorAsync("已完成任务加载失败", exception);
        }
        finally
        {
            QuadrantsNavigationItem.IsActive = true;
        }
    }

    private void Settings_Click(object sender, RoutedEventArgs e)
    {
        SettingsRequested?.Invoke(this, EventArgs.Empty);
        QuadrantsNavigationItem.IsActive = true;
    }

    private async void NewTaskRequested(object? sender, EventArgs e)
    {
        try
        {
            var viewModel = (MainViewModel)DataContext;
            var editor = new TaskEditorWindow(new TaskEditorViewModel(viewModel.Quadrants.Select(ToDefinition), viewModel.Clock)) { Owner = this };
            if (editor.ShowDialog() == true && editor.DraftResult is { } draft)
            {
                await viewModel.CreateAsync(draft);
                ShowFeedback("任务已添加", draft.Title);
            }
        }
        catch (Exception exception)
        {
            await ShowRecoverableErrorAsync("任务保存失败", exception);
        }
    }

    private async void EditTaskRequested(object? sender, TaskItem task)
    {
        try
        {
            var viewModel = (MainViewModel)DataContext;
            var editor = new TaskEditorWindow(new TaskEditorViewModel(viewModel.Quadrants.Select(ToDefinition), viewModel.Clock, task)) { Owner = this };
            if (editor.ShowDialog() == true && editor.UpdateResult is { } update)
            {
                await viewModel.UpdateAsync(update);
                ShowFeedback("任务已更新", update.Title);
            }
        }
        catch (Exception exception)
        {
            await ShowRecoverableErrorAsync("任务保存失败", exception);
        }
    }

    private async void ViewModel_RecoverableError(object? sender, RecoverableOperationErrorEventArgs e) =>
        await ShowRecoverableErrorAsync(e.Title, e.Exception);

    public async Task ActivateAndOpenTaskAsync(long id)
    {
        ShowFromTray();
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
                if (IsVisible)
                {
                    ShowFeedback("任务已添加", draft.Title);
                }
            }
        }
        finally
        {
            quickAddOpen = false;
        }
    }

    private async void DeleteTaskRequested(object? sender, long id)
    {
        var result = await ShowDialogAsync(
            "删除任务？",
            "此操作会永久删除该任务。",
            "删除",
            ControlAppearance.Danger);
        if (result != ContentDialogResult.Primary)
        {
            return;
        }

        try
        {
            await ((MainViewModel)DataContext).ConfirmedDeleteAsync(id);
            ShowFeedback("任务已删除", "任务已从四象限中移除。", ControlAppearance.Success, SymbolRegular.Delete24);
        }
        catch (Exception exception)
        {
            await ShowRecoverableErrorAsync("任务删除失败", exception);
        }
    }

    private async Task ShowRecoverableErrorAsync(string title, Exception exception)
    {
        if (!IsVisible)
        {
            System.Windows.MessageBox.Show(
                $"{title}。\n{exception.Message}",
                title,
                System.Windows.MessageBoxButton.OK,
                System.Windows.MessageBoxImage.Warning);
            return;
        }

        await ShowDialogAsync(title, exception.Message, "知道了", ControlAppearance.Primary, closeButtonText: null);
    }

    private async Task<ContentDialogResult> ShowDialogAsync(
        string title,
        string message,
        string primaryButtonText,
        ControlAppearance primaryAppearance,
        string? closeButtonText = "取消")
    {
        if (dialogOpen)
        {
            ShowFeedback(title, message, ControlAppearance.Caution, SymbolRegular.Alert24);
            return ContentDialogResult.None;
        }

        dialogOpen = true;
        try
        {
            var dialog = new ContentDialog(DialogHost)
            {
                Title = title,
                Content = new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap },
                PrimaryButtonText = primaryButtonText,
                CloseButtonText = closeButtonText ?? string.Empty,
                PrimaryButtonAppearance = primaryAppearance,
                DefaultButton = ContentDialogButton.Primary
            };
            return await dialog.ShowAsync();
        }
        finally
        {
            dialogOpen = false;
        }
    }

    private static QuadrantDefinition ToDefinition(QuadrantViewModel quadrant) =>
        new(quadrant.Id, quadrant.Name, quadrant.Subtitle);
}
