namespace Quadrant.App;

public partial class App : System.Windows.Application
{
    private readonly Quadrant.Infrastructure.Logging.DiagnosticLogger diagnosticLogger =
        new(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData));
    private readonly Quadrant.Infrastructure.Notifications.WindowsAppNotificationService notificationService = new();
    private readonly Quadrant.Infrastructure.Notifications.WindowsReminderScheduler reminderScheduler = new();
    private readonly Quadrant.Infrastructure.Windows.SingleInstanceService singleInstanceService = new();
    private readonly Quadrant.Infrastructure.Windows.GlobalHotkeyService globalHotkeyService = new();
    private readonly Quadrant.Infrastructure.Windows.TrayService trayService = new();
    private readonly ShutdownCoordinator shutdownCoordinator = new();
    private Quadrant.Infrastructure.Storage.SqliteSettingsRepository? settingsRepository;
    private Quadrant.Infrastructure.Windows.RegistryStartupService? startupService;
    private Quadrant.Core.Models.AppSettings? currentSettings;
    private System.Drawing.Icon? trayIcon;
    private Quadrant.Infrastructure.Notifications.NotificationActivation? pendingActivation;
    private bool isWatchingSystemTheme;

    private async void OnStartup(object sender, System.Windows.StartupEventArgs e)
    {
        var startupTimer = System.Diagnostics.Stopwatch.StartNew();
        ShutdownMode = System.Windows.ShutdownMode.OnExplicitShutdown;
        var startInBackground = e.Args.Any(argument => string.Equals(argument, "--background", StringComparison.OrdinalIgnoreCase));
        notificationService.ActivationReceived += NotificationService_ActivationReceived;
        try
        {
            notificationService.Register();
        }
        catch (Exception exception)
        {
            diagnosticLogger.Warning("App notification registration failed; continuing without immediate notification registration.", exception);
        }

        var activationArgs = Microsoft.Windows.AppLifecycle.AppInstance.GetCurrent().GetActivatedEventArgs();
        if (!singleInstanceService.RegisterCurrentInstance(SingleInstance_Activated))
        {
            await singleInstanceService.RedirectActivationAsync(activationArgs);
            Shutdown();
            return;
        }

        if (activationArgs.Kind != Microsoft.Windows.AppLifecycle.ExtendedActivationKind.Launch)
        {
            pendingActivation = ParseActivation(activationArgs);
        }

#if DEBUG
        var windowsAppSdkProbe = new Quadrant.Infrastructure.Windows.WindowsAppSdkEnvironmentProbe().Probe();
        System.Diagnostics.Debug.WriteLine(
            windowsAppSdkProbe.IsAvailable
                ? $"Windows App SDK runtime: {windowsAppSdkProbe.RuntimeVersion}"
                : $"Windows App SDK runtime unavailable: {windowsAppSdkProbe.ErrorType}: {windowsAppSdkProbe.ErrorMessage}");
#endif

        var pathProvider = new Quadrant.Infrastructure.Storage.LocalAppDataPathProvider();
        var connectionFactory = new Quadrant.Infrastructure.Storage.SqliteConnectionFactory(pathProvider.DatabasePath);
        var initializer = new Quadrant.Infrastructure.Storage.SqliteDatabaseInitializer(connectionFactory);
        try
        {
            await initializer.InitializeAsync();
            settingsRepository = new Quadrant.Infrastructure.Storage.SqliteSettingsRepository(connectionFactory);
            currentSettings = await settingsRepository.GetAsync();
        }
        catch (Exception exception)
        {
            diagnosticLogger.Error($"Database initialization or settings load failed. Database path: {pathProvider.DatabasePath}", exception);
            System.Windows.MessageBox.Show(
                $"数据库初始化失败，应用无法继续写入。\n数据文件：{pathProvider.DatabasePath}\n\n{exception.Message}",
                "数据库不可用",
                System.Windows.MessageBoxButton.OK,
                System.Windows.MessageBoxImage.Error);
            Shutdown();
            return;
        }
        startupService = new Quadrant.Infrastructure.Windows.RegistryStartupService();

        var taskRepository = new Quadrant.Infrastructure.Storage.SqliteTaskRepository(connectionFactory);
        var quadrantRepository = new Quadrant.Infrastructure.Storage.SqliteQuadrantRepository(connectionFactory);
        var appChangeHub = new Quadrant.Core.Services.AppChangeHub(diagnosticLogger);
        var taskService = new Quadrant.Core.Services.TaskService(
            taskRepository,
            reminderScheduler,
            new Quadrant.Infrastructure.Windows.SystemClock(),
            diagnosticLogger,
            appChangeHub);
        var clock = new Quadrant.Infrastructure.Windows.SystemClock();
        var todayQueryService = new Quadrant.Core.Services.TodayQueryService(taskRepository, clock);
        var viewModel = new ViewModels.MainViewModel(taskService, quadrantRepository, clock, appChangeHub, todayQueryService);
        try
        {
            await viewModel.LoadAsync();
        }
        catch (Exception exception)
        {
            diagnosticLogger.Error("Initial task load failed; refusing to show an incomplete workspace.", exception);
            System.Windows.MessageBox.Show(
                $"任务数据加载失败，应用无法安全继续。\n\n{exception.Message}",
                "数据加载失败",
                System.Windows.MessageBoxButton.OK,
                System.Windows.MessageBoxImage.Error);
            Shutdown();
            return;
        }

        try
        {
            var reconciliation = await reminderScheduler.ReconcileAsync(viewModel.ActiveTasks, clock.LocalNow);
            viewModel.SetPossiblyMissedReminders(reconciliation.Tasks);
        }
        catch (Exception exception)
        {
            diagnosticLogger.Warning("Reminder reconciliation failed; the database remains available.", exception);
        }

        var mainWindow = new Views.MainWindow
        {
            DataContext = viewModel
        };

        MainWindow = mainWindow;
        mainWindow.ConfigureGlobalHotkey(globalHotkeyService);
        mainWindow.GlobalHotkeyPressed += GlobalHotkeyService_HotkeyPressed;
        mainWindow.SettingsRequested += MainWindow_SettingsRequested;
        mainWindow.ExitRequested += MainWindow_ExitRequested;
        globalHotkeyService.RegistrationFailed += GlobalHotkeyService_RegistrationFailed;
        trayService.ShowRequested += TrayService_ShowRequested;
        trayService.QuickAddRequested += TrayService_QuickAddRequested;
        trayService.ExitRequested += TrayService_ExitRequested;
        try
        {
            trayIcon = LoadTrayIcon();
            trayService.Initialize(trayIcon);
        }
        catch (Exception exception)
        {
            diagnosticLogger.Warning("Tray initialization failed; continuing without a tray icon.", exception);
            trayIcon?.Dispose();
            trayIcon = null;
        }
        mainWindow.IsCloseToTray = currentSettings.CloseToTray;
        // EnsureHandle creates the HWND and raises SourceInitialized without making
        // the managed window visible, so background startup can register the global
        // hotkey without flashing or stealing foreground focus.
        _ = new System.Windows.Interop.WindowInteropHelper(mainWindow).EnsureHandle();
        // WPF UI updates the main window's theme resources and backdrop when a theme
        // is applied. Do this only after the persisted-theme window has an HWND;
        // applying it before MainWindow/its handle exists leaves some dynamic text
        // resources at the initial Light values until the user changes theme again.
        ApplyTheme(currentSettings.Theme);
        ConfigureSystemThemeWatcher(mainWindow, currentSettings.Theme);
        if (!startInBackground && !currentSettings.StartMinimized)
        {
            mainWindow.Show();
        }
        startupTimer.Stop();
        System.Diagnostics.Debug.WriteLine($"Cold start completed in {startupTimer.ElapsedMilliseconds} ms.");

        if (pendingActivation is not null)
        {
            await HandleActivationAsync(pendingActivation);
            pendingActivation = null;
        }
    }

    protected override void OnExit(System.Windows.ExitEventArgs e)
    {
        if (isWatchingSystemTheme && MainWindow is System.Windows.Window window)
        {
            Wpf.Ui.Appearance.SystemThemeWatcher.UnWatch(window);
            isWatchingSystemTheme = false;
        }

        trayService.Dispose();
        trayIcon?.Dispose();
        trayIcon = null;
        globalHotkeyService.Dispose();
        notificationService.Dispose();
        singleInstanceService.Dispose();
        base.OnExit(e);
    }

    private void TrayService_ShowRequested(object? sender, EventArgs e)
    {
        if (MainWindow is Views.MainWindow mainWindow)
        {
            mainWindow.ShowFromTray();
        }
    }

    private void TrayService_QuickAddRequested(object? sender, EventArgs e)
    {
        if (MainWindow is Views.MainWindow mainWindow)
        {
            _ = Dispatcher.InvokeAsync(() => mainWindow.ShowQuickAddAsync());
        }
    }

    private async void MainWindow_SettingsRequested(object? sender, EventArgs e)
    {
        if (MainWindow is not Views.MainWindow mainWindow || settingsRepository is null || currentSettings is null || startupService is null)
        {
            return;
        }

        var viewModel = (ViewModels.MainViewModel)mainWindow.DataContext;
        var settingsWindow = new Views.SettingsWindow(new ViewModels.SettingsViewModel(
            currentSettings,
            viewModel.Quadrants.Select(quadrant => new Quadrant.Core.Models.QuadrantDefinition(quadrant.Id, quadrant.Name, quadrant.Subtitle))))
        {
            Owner = mainWindow
        };

        if (settingsWindow.ShowDialog() == true &&
            settingsWindow.DesiredSettings is { } desiredSettings &&
            settingsWindow.DesiredQuadrants is { } desiredQuadrants)
        {
            var previousSettings = currentSettings;
            try
            {
                // Apply the external derived state first. If DB persistence fails,
                // restore the previous registry state so the UI and startup entry do
                // not silently disagree.
                startupService.SetEnabled(desiredSettings.LaunchAtStartup, desiredSettings.StartMinimized);
                await settingsRepository.SaveAsync(desiredSettings, desiredQuadrants);
                currentSettings = desiredSettings;
                ApplyTheme(currentSettings.Theme);
                ConfigureSystemThemeWatcher(mainWindow, currentSettings.Theme);
                mainWindow.IsCloseToTray = currentSettings.CloseToTray;
                viewModel.UpdateDefinitions(desiredQuadrants);
                mainWindow.ShowFeedback("设置已保存", "主题和四象限设置已更新。");
            }
            catch (Exception exception)
            {
                try
                {
                    startupService.SetEnabled(previousSettings.LaunchAtStartup, previousSettings.StartMinimized);
                }
                catch (Exception rollbackException)
                {
                    diagnosticLogger.Warning("Startup setting rollback failed.", rollbackException);
                }

                diagnosticLogger.Warning("Applying settings failed; keeping the current session alive.", exception);
                System.Windows.MessageBox.Show(exception.Message, "设置应用失败", System.Windows.MessageBoxButton.OK, System.Windows.MessageBoxImage.Warning);
            }
        }
    }

    private static void ApplyTheme(string theme)
    {
        if (string.Equals(theme, "System", StringComparison.OrdinalIgnoreCase))
        {
            Wpf.Ui.Appearance.ApplicationThemeManager.ApplySystemTheme(updateAccent: true);
            return;
        }

        var applicationTheme = string.Equals(theme, "Dark", StringComparison.OrdinalIgnoreCase)
            ? Wpf.Ui.Appearance.ApplicationTheme.Dark
            : Wpf.Ui.Appearance.ApplicationTheme.Light;
        Wpf.Ui.Appearance.ApplicationThemeManager.Apply(
            applicationTheme,
            Wpf.Ui.Controls.WindowBackdropType.Mica,
            updateAccent: true);
    }

    private void ConfigureSystemThemeWatcher(System.Windows.Window window, string theme)
    {
        if (isWatchingSystemTheme)
        {
            Wpf.Ui.Appearance.SystemThemeWatcher.UnWatch(window);
            isWatchingSystemTheme = false;
        }

        if (string.Equals(theme, "System", StringComparison.OrdinalIgnoreCase))
        {
            Wpf.Ui.Appearance.SystemThemeWatcher.Watch(
                window,
                Wpf.Ui.Controls.WindowBackdropType.Mica,
                updateAccents: true);
            isWatchingSystemTheme = true;
        }
    }

    private void TrayService_ExitRequested(object? sender, EventArgs e) => ExitApplication();

    private void MainWindow_ExitRequested(object? sender, EventArgs e) => ExitApplication();

    private void ExitApplication()
    {
        if (shutdownCoordinator.IsExiting)
        {
            return;
        }

        shutdownCoordinator.BeginExit();
        if (MainWindow is Views.MainWindow mainWindow)
        {
            mainWindow.AllowApplicationExit();
            mainWindow.Close();
        }

        Shutdown();
    }

    private static System.Drawing.Icon LoadTrayIcon()
    {
        var resource = GetResourceStream(
            new Uri("pack://application:,,,/Resources/Quadrant.ico", UriKind.Absolute))
            ?? throw new InvalidOperationException("托盘图标资源缺失。");
        using (resource.Stream)
        {
            return new System.Drawing.Icon(resource.Stream);
        }
    }

    private void GlobalHotkeyService_HotkeyPressed(object? sender, EventArgs e)
    {
        if (MainWindow is Views.MainWindow mainWindow)
        {
            _ = Dispatcher.InvokeAsync(async () =>
            {
                try
                {
                    await mainWindow.ShowQuickAddAsync();
                }
                catch (Exception exception)
                {
                    diagnosticLogger.Warning("Quick Add failed.", exception);
                }
            });
        }
    }

    private static void GlobalHotkeyService_RegistrationFailed(object? sender, Quadrant.Infrastructure.Windows.GlobalHotkeyRegistrationFailedEventArgs e)
    {
        System.Windows.MessageBox.Show(
            $"快速添加快捷键 Ctrl+Alt+Q 注册失败，应用仍可正常使用。\n{e.Message}",
            "快捷键不可用",
            System.Windows.MessageBoxButton.OK,
            System.Windows.MessageBoxImage.Information);
    }

    private void NotificationService_ActivationReceived(
        object? sender,
        Quadrant.Infrastructure.Notifications.NotificationActivation activation)
    {
        _ = Dispatcher.InvokeAsync(() => HandleActivationAsync(activation));
    }

    private void SingleInstance_Activated(
        object? sender,
        Microsoft.Windows.AppLifecycle.AppActivationArguments arguments)
    {
        // AppInstance may raise Activated away from WPF's UI thread. Marshal the
        // whole decision, including MainWindow access, to the Dispatcher.
        _ = Dispatcher.InvokeAsync(async () =>
        {
            try
            {
                var activation = ParseActivation(arguments);
                if (activation is not null)
                {
                    await HandleActivationAsync(activation);
                }
                else if (MainWindow is Views.MainWindow mainWindow)
                {
                    mainWindow.ShowFromTray();
                }
            }
            catch (Exception exception)
            {
                diagnosticLogger.Warning("Redirected activation failed.", exception);
            }
        });
    }

    private async Task HandleActivationAsync(
        Quadrant.Infrastructure.Notifications.NotificationActivation activation)
    {
        if (MainWindow is not Views.MainWindow mainWindow || mainWindow.DataContext is not ViewModels.MainViewModel viewModel)
        {
            pendingActivation = activation;
            return;
        }

        try
        {
            if (activation.Action == "complete")
            {
                await viewModel.CompleteFromNotificationAsync(activation.TaskId);
            }
            else if (activation.Action == "snooze10")
            {
                await viewModel.SnoozeFromNotificationAsync(activation.TaskId);
            }
            else
            {
                await mainWindow.ActivateAndOpenTaskAsync(activation.TaskId);
            }
        }
        catch (Exception exception)
        {
            diagnosticLogger.Warning("Notification activation failed.", exception);
        }
    }

    private static Quadrant.Infrastructure.Notifications.NotificationActivation? ParseActivation(
        Microsoft.Windows.AppLifecycle.AppActivationArguments arguments)
    {
        return Quadrant.Infrastructure.Notifications.NotificationActivationParser.TryParse(
            (arguments.Data as Microsoft.Windows.AppNotifications.AppNotificationActivatedEventArgs)?.Argument,
            out var activation)
            ? activation
            : null;
    }
}
