using CommunityToolkit.Mvvm.ComponentModel;

namespace Quadrant.App.ViewModels;

public partial class MainViewModel : ObservableObject
{
    [ObservableProperty]
    private string appTitle = "Quadrant";

    [ObservableProperty]
    private string placeholderTitle = "四象限任务工作区";
}
