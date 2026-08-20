using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public partial class MainViewModel : ObservableObject
{
    private readonly ITaskService taskService;
    private readonly IQuadrantRepository quadrantRepository;

    public MainViewModel(ITaskService taskService, IQuadrantRepository quadrantRepository)
    {
        this.taskService = taskService ?? throw new ArgumentNullException(nameof(taskService));
        this.quadrantRepository = quadrantRepository ?? throw new ArgumentNullException(nameof(quadrantRepository));
    }

    [ObservableProperty]
    private string appTitle = "Quadrant";

    [ObservableProperty]
    private string placeholderTitle = "四象限任务工作区";

    public IReadOnlyList<QuadrantViewModel> Quadrants { get; private set; } = [];

    public async Task LoadAsync(CancellationToken cancellationToken = default)
    {
        var definitions = await quadrantRepository.GetAllAsync(cancellationToken);
        var activeTasks = await taskService.GetActiveAsync(cancellationToken);

        Quadrants = definitions
            .OrderBy(definition => definition.Id)
            .Select(definition => new QuadrantViewModel(
                definition,
                activeTasks.Where(task => task.QuadrantId == definition.Id)))
            .ToArray();

        OnPropertyChanged(nameof(Quadrants));
    }
}
