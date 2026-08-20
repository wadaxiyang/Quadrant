namespace Quadrant.App;

public partial class App : System.Windows.Application
{
    protected override async void OnStartup(System.Windows.StartupEventArgs e)
    {
        base.OnStartup(e);

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
        var viewModel = new ViewModels.MainViewModel(taskService, quadrantRepository);
        await viewModel.LoadAsync();

        var mainWindow = new Views.MainWindow
        {
            DataContext = viewModel
        };

        MainWindow = mainWindow;
        mainWindow.Show();
    }
}
