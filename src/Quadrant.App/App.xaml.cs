namespace Quadrant.App;

public partial class App : System.Windows.Application
{
    private readonly Quadrant.Infrastructure.Notifications.WindowsAppNotificationService notificationService = new();
    private readonly Quadrant.Infrastructure.Windows.SingleInstanceService singleInstanceService = new();
    private Quadrant.Infrastructure.Notifications.NotificationActivation? pendingActivation;

    private async void OnStartup(object sender, System.Windows.StartupEventArgs e)
    {
        notificationService.ActivationReceived += NotificationService_ActivationReceived;
        try
        {
            notificationService.Register();
        }
        catch (Exception exception)
        {
            System.Diagnostics.Debug.WriteLine($"App notification registration failed: {exception}");
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
        await initializer.InitializeAsync();

        var taskRepository = new Quadrant.Infrastructure.Storage.SqliteTaskRepository(connectionFactory);
        var quadrantRepository = new Quadrant.Infrastructure.Storage.SqliteQuadrantRepository(connectionFactory);
        var taskService = new Quadrant.Core.Services.TaskService(
            taskRepository,
            new Quadrant.Infrastructure.Notifications.NoOpReminderScheduler(),
            new Quadrant.Infrastructure.Windows.SystemClock());
        var clock = new Quadrant.Infrastructure.Windows.SystemClock();
        var viewModel = new ViewModels.MainViewModel(taskService, quadrantRepository, clock);
        await viewModel.LoadAsync();

        var mainWindow = new Views.MainWindow
        {
            DataContext = viewModel
        };

        MainWindow = mainWindow;
        mainWindow.Show();

        if (pendingActivation is not null)
        {
            await HandleActivationAsync(pendingActivation);
            pendingActivation = null;
        }
    }

    protected override void OnExit(System.Windows.ExitEventArgs e)
    {
        notificationService.Dispose();
        singleInstanceService.Dispose();
        base.OnExit(e);
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
        var activation = ParseActivation(arguments);
        if (activation is not null)
        {
            _ = Dispatcher.InvokeAsync(() => HandleActivationAsync(activation));
        }
        else if (MainWindow is not null)
        {
            _ = Dispatcher.InvokeAsync(() => MainWindow.Activate());
        }
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
            else
            {
                await mainWindow.ActivateAndOpenTaskAsync(activation.TaskId);
            }
        }
        catch (Exception exception)
        {
            System.Diagnostics.Debug.WriteLine($"Notification activation failed: {exception}");
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
