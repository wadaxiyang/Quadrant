using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.App.ViewModels;

public sealed record SettingsChoice<T>(T Value, string Label);

public partial class SettingsViewModel : ObservableObject
{
    private readonly IDataMaintenanceService? dataMaintenanceService;

    public SettingsViewModel(AppSettings settings, IEnumerable<QuadrantDefinition> quadrants, string databasePath = "", IDataMaintenanceService? dataMaintenanceService = null)
    {
        this.dataMaintenanceService = dataMaintenanceService;
        Theme = settings.Theme;
        CloseToTray = settings.CloseToTray;
        LaunchAtStartup = settings.LaunchAtStartup;
        StartMinimized = settings.StartMinimized;
        GlobalHotkey = settings.GlobalHotkey;
        QuickCaptureQuadrantId = settings.QuickCaptureQuadrantId;
        DefaultReminder = settings.DefaultReminder;
        FocusMinutes = settings.FocusMinutes;
        ShortBreakMinutes = settings.ShortBreakMinutes;
        LongBreakMinutes = settings.LongBreakMinutes;
        LongBreakInterval = settings.LongBreakInterval;
        AutoStartBreak = settings.AutoStartBreak;
        AutoStartFocus = settings.AutoStartFocus;
        TaskRemindersEnabled = settings.TaskRemindersEnabled;
        FocusNotificationsEnabled = settings.FocusNotificationsEnabled;
        NotificationSoundEnabled = settings.NotificationSoundEnabled;
        ReviewDefaultRange = settings.ReviewDefaultRange;
        WeekStart = settings.WeekStart;
        SidebarIconSize = settings.SidebarIconSize;
        CollapseSidebarOnStartup = settings.CollapseSidebarOnStartup;
        DatabasePath = databasePath;
        Quadrants = quadrants.OrderBy(item => item.Id).Select(item => new EditableQuadrantViewModel(item)).ToArray();
    }

    public IReadOnlyList<EditableQuadrantViewModel> Quadrants { get; }

    [ObservableProperty] public partial string Theme { get; set; }
    [ObservableProperty] public partial bool CloseToTray { get; set; }
    [ObservableProperty] public partial bool LaunchAtStartup { get; set; }
    [ObservableProperty] public partial bool StartMinimized { get; set; }
    [ObservableProperty] public partial string GlobalHotkey { get; set; }
    [ObservableProperty] public partial int? QuickCaptureQuadrantId { get; set; }
    [ObservableProperty] public partial ReminderPreset DefaultReminder { get; set; }
    [ObservableProperty] public partial int FocusMinutes { get; set; }
    [ObservableProperty] public partial int ShortBreakMinutes { get; set; }
    [ObservableProperty] public partial int LongBreakMinutes { get; set; }
    [ObservableProperty] public partial int LongBreakInterval { get; set; }
    [ObservableProperty] public partial bool AutoStartBreak { get; set; }
    [ObservableProperty] public partial bool AutoStartFocus { get; set; }
    [ObservableProperty] public partial bool TaskRemindersEnabled { get; set; }
    [ObservableProperty] public partial bool FocusNotificationsEnabled { get; set; }
    [ObservableProperty] public partial bool NotificationSoundEnabled { get; set; }
    [ObservableProperty] public partial ReviewRange ReviewDefaultRange { get; set; }
    [ObservableProperty] public partial DayOfWeek WeekStart { get; set; }
    [ObservableProperty] public partial double SidebarIconSize { get; set; }
    [ObservableProperty] public partial bool CollapseSidebarOnStartup { get; set; }

    public string DatabasePath { get; }
    public bool HasDataMaintenance => dataMaintenanceService is not null;
    public IReadOnlyList<SettingsChoice<int?>> QuickCaptureDestinations { get; } =
    [new(null, "Inbox"), new(1, "Q1"), new(2, "Q2"), new(3, "Q3"), new(4, "Q4")];
    public IReadOnlyList<SettingsChoice<ReminderPreset>> ReminderChoices { get; } =
    [new(ReminderPreset.None, "不提醒"), new(ReminderPreset.AtDueTime, "到期时"), new(ReminderPreset.TenMinutesBefore, "提前 10 分钟"), new(ReminderPreset.OneHourBefore, "提前 1 小时"), new(ReminderPreset.OneDayBefore, "提前 1 天")];
    public IReadOnlyList<SettingsChoice<ReviewRange>> ReviewRangeChoices { get; } =
    [new(ReviewRange.SevenDays, "7 天"), new(ReviewRange.ThirtyDays, "30 天"), new(ReviewRange.NinetyDays, "90 天"), new(ReviewRange.AllTime, "全部")];
    public IReadOnlyList<SettingsChoice<DayOfWeek>> WeekStartChoices { get; } =
    [new(DayOfWeek.Monday, "星期一"), new(DayOfWeek.Sunday, "星期日")];

    public Task BackupAsync(string path, CancellationToken cancellationToken = default) => RequireDataService().BackupAsync(path, cancellationToken);
    public Task ExportJsonAsync(string path, CancellationToken cancellationToken = default) => RequireDataService().ExportJsonAsync(path, cancellationToken);
    public Task ClearFocusHistoryAsync(CancellationToken cancellationToken = default) => RequireDataService().ClearFocusHistoryAsync(cancellationToken);
    public Task ClearCompletionHistoryAsync(CancellationToken cancellationToken = default) => RequireDataService().ClearCompletionHistoryAsync(cancellationToken);
    public Task ResetAllAsync(CancellationToken cancellationToken = default) => RequireDataService().ResetAllAsync(cancellationToken);

    public AppSettings BuildSettings()
    {
        Validate();
        var settings = new AppSettings(
            Theme, CloseToTray, LaunchAtStartup, StartMinimized, GlobalHotkey.Trim(),
            QuickCaptureQuadrantId, DefaultReminder, FocusMinutes, ShortBreakMinutes,
            LongBreakMinutes, LongBreakInterval, AutoStartBreak, AutoStartFocus,
            TaskRemindersEnabled, FocusNotificationsEnabled, NotificationSoundEnabled,
            ReviewDefaultRange, WeekStart, SidebarIconSize, CollapseSidebarOnStartup);
        settings.Validate();
        return settings;
    }

    public IReadOnlyList<QuadrantDefinition> BuildQuadrants()
    {
        Validate();
        return Quadrants
            .Select(quadrant => new QuadrantDefinition(quadrant.Id, quadrant.Name.Trim(), quadrant.Subtitle.Trim()))
            .ToArray();
    }

    private void Validate()
    {
        if (Theme is not ("System" or "Light" or "Dark"))
        {
            throw new InvalidOperationException("请选择有效主题。");
        }

        if (!string.Equals(GlobalHotkey.Trim(), "Ctrl+Alt+Q", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidOperationException("当前支持的快捷键为 Ctrl+Alt+Q。");
        }

        if (Quadrants.Any(item => string.IsNullOrWhiteSpace(item.Name) || string.IsNullOrWhiteSpace(item.Subtitle)))
        {
            throw new InvalidOperationException("象限名称和副标题不能为空。");
        }

        new AppSettings(
            Theme, CloseToTray, LaunchAtStartup, StartMinimized, GlobalHotkey.Trim(),
            QuickCaptureQuadrantId, DefaultReminder, FocusMinutes, ShortBreakMinutes,
            LongBreakMinutes, LongBreakInterval, AutoStartBreak, AutoStartFocus,
            TaskRemindersEnabled, FocusNotificationsEnabled, NotificationSoundEnabled,
            ReviewDefaultRange, WeekStart, SidebarIconSize, CollapseSidebarOnStartup).Validate();
    }

    private IDataMaintenanceService RequireDataService() => dataMaintenanceService ?? throw new InvalidOperationException("数据维护服务不可用。");
}

public partial class EditableQuadrantViewModel : ObservableObject
{
    public EditableQuadrantViewModel(QuadrantDefinition definition)
    {
        Id = definition.Id;
        Name = definition.Name;
        Subtitle = definition.Subtitle;
    }

    public int Id { get; }
    [ObservableProperty] public partial string Name { get; set; }
    [ObservableProperty] public partial string Subtitle { get; set; }
}
