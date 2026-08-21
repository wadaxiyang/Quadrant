using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public partial class SettingsViewModel : ObservableObject
{
    private readonly ISettingsRepository settingsRepository;
    private readonly IQuadrantRepository quadrantRepository;

    public SettingsViewModel(ISettingsRepository settingsRepository, IQuadrantRepository quadrantRepository, AppSettings settings, IEnumerable<QuadrantDefinition> quadrants)
    {
        this.settingsRepository = settingsRepository;
        this.quadrantRepository = quadrantRepository;
        Theme = settings.Theme;
        CloseToTray = settings.CloseToTray;
        LaunchAtStartup = settings.LaunchAtStartup;
        StartMinimized = settings.StartMinimized;
        GlobalHotkey = settings.GlobalHotkey;
        Quadrants = quadrants.OrderBy(item => item.Id).Select(item => new EditableQuadrantViewModel(item)).ToArray();
    }

    public IReadOnlyList<EditableQuadrantViewModel> Quadrants { get; }
    [ObservableProperty] public partial string Theme { get; set; }
    [ObservableProperty] public partial bool CloseToTray { get; set; }
    [ObservableProperty] public partial bool LaunchAtStartup { get; set; }
    [ObservableProperty] public partial bool StartMinimized { get; set; }
    [ObservableProperty] public partial string GlobalHotkey { get; set; }

    public async Task SaveAsync(CancellationToken cancellationToken = default)
    {
        if (!string.Equals(GlobalHotkey.Trim(), "Ctrl+Alt+Q", StringComparison.OrdinalIgnoreCase)) throw new InvalidOperationException("当前支持的快捷键为 Ctrl+Alt+Q。");
        if (Quadrants.Any(item => string.IsNullOrWhiteSpace(item.Name) || string.IsNullOrWhiteSpace(item.Subtitle))) throw new InvalidOperationException("象限名称和副标题不能为空。");
        await settingsRepository.SaveAsync(new AppSettings(Theme, CloseToTray, LaunchAtStartup, StartMinimized, GlobalHotkey.Trim()), cancellationToken);
        foreach (var quadrant in Quadrants) await quadrantRepository.UpdateAsync(new QuadrantDefinition(quadrant.Id, quadrant.Name.Trim(), quadrant.Subtitle.Trim()), cancellationToken);
    }
}

public partial class EditableQuadrantViewModel : ObservableObject
{
    public EditableQuadrantViewModel(QuadrantDefinition definition) { Id = definition.Id; Name = definition.Name; Subtitle = definition.Subtitle; }
    public int Id { get; }
    [ObservableProperty] public partial string Name { get; set; }
    [ObservableProperty] public partial string Subtitle { get; set; }
}
