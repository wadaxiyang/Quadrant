using CommunityToolkit.Mvvm.ComponentModel;
using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;
using Quadrant.Core.Services;

namespace Quadrant.App.ViewModels;

public partial class FocusPageViewModel : ObservableObject
{
    private readonly IFocusTimerService stopwatch;
    private readonly PomodoroTimerService pomodoro;
    private readonly IFocusSessionService sessions;
    private readonly PomodoroSettings settings;
    private readonly DateOnly localDate;
    private readonly HashSet<long> todayTaskIds;

    public static async Task<FocusPageViewModel> CreateAsync(
        ITaskService taskService,
        ITodayQueryService todayQueryService,
        IFocusTimerService stopwatch,
        PomodoroTimerService pomodoro,
        IFocusSessionService sessions,
        PomodoroSettings settings,
        IClock clock,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(taskService);
        ArgumentNullException.ThrowIfNull(todayQueryService);
        ArgumentNullException.ThrowIfNull(clock);

        var activeTask = taskService.GetActiveAsync(cancellationToken);
        var inboxTask = taskService.GetInboxAsync(cancellationToken: cancellationToken);
        var todayTask = todayQueryService.GetSnapshotAsync(cancellationToken);
        await Task.WhenAll(activeTask, inboxTask, todayTask);

        var today = await todayTask;
        var todayTaskIds = today.Overdue.Concat(today.PlannedToday).Concat(today.DueToday).Concat(today.NeedsReschedule)
            .Select(task => task.Id);
        var tasks = (await activeTask).Concat(await inboxTask).DistinctBy(task => task.Id).ToArray();
        return new FocusPageViewModel(tasks, stopwatch, pomodoro, sessions, settings, clock.LocalDate, todayTaskIds);
    }

    public FocusPageViewModel(IReadOnlyList<TaskItem> tasks, IFocusTimerService stopwatch, PomodoroTimerService pomodoro,
        IFocusSessionService sessions, PomodoroSettings? settings = null, DateOnly? localDate = null,
        IEnumerable<long>? todayTaskIds = null)
    {
        ArgumentNullException.ThrowIfNull(tasks);
        this.stopwatch = stopwatch ?? throw new ArgumentNullException(nameof(stopwatch));
        this.pomodoro = pomodoro ?? throw new ArgumentNullException(nameof(pomodoro));
        this.sessions = sessions ?? throw new ArgumentNullException(nameof(sessions));
        this.settings = settings ?? new PomodoroSettings();
        this.localDate = localDate ?? DateOnly.FromDateTime(DateTime.Today);
        this.todayTaskIds = todayTaskIds?.ToHashSet() ?? [];

        Tasks = tasks.Where(task => !task.IsCompleted).DistinctBy(task => task.Id).ToArray();
        TaskOptions = Tasks.Select(task => new FocusTaskOption(task, task.Title, FormatTaskMetadata(task, this.localDate))).ToArray();
        TaskSources =
        [
            new(FocusTaskSource.Inbox, "Inbox", "未分类任务"),
            new(FocusTaskSource.Today, "Today", "今天需要处理"),
            new(FocusTaskSource.Quadrant1, "Q1", "重要且紧急"),
            new(FocusTaskSource.Quadrant2, "Q2", "重要不紧急"),
            new(FocusTaskSource.Quadrant3, "Q3", "紧急不重要"),
            new(FocusTaskSource.Quadrant4, "Q4", "不重要不紧急")
        ];
        SelectedTaskSource = TaskSources[1];
        ApplyIdleTimer();
    }

    public IReadOnlyList<TaskItem> Tasks { get; }
    public IReadOnlyList<FocusTaskOption> TaskOptions { get; }
    public IReadOnlyList<FocusTaskSourceOption> TaskSources { get; }
    public TaskItem? SelectedTask => SelectedTaskOption?.Task;
    public IReadOnlyList<FocusTaskOption> FilteredTaskOptions => TaskOptions.Where(IsInSelectedSource).ToArray();
    public bool HasFilteredTasks => FilteredTaskOptions.Count > 0;
    public bool IsFilteredTaskListEmpty => !HasFilteredTasks;
    public bool HasSelectedTask => SelectedTaskOption is not null;
    public string SelectedTaskTitle => SelectedTaskOption?.Title ?? "未关联任务";
    public string SelectedTaskMetadata => SelectedTaskOption?.Metadata ?? "从右侧选择任务，或直接开始临时专注";
    public string SelectedSourceDescription => $"{SelectedTaskSource.Description} · {FilteredTaskOptions.Count} 项";

    [ObservableProperty] public partial FocusTaskOption? SelectedTaskOption { get; private set; }
    [ObservableProperty] public partial FocusTaskSourceOption SelectedTaskSource { get; set; }
    [ObservableProperty] public partial FocusMode Mode { get; set; } = FocusMode.Pomodoro;
    [ObservableProperty] public partial string TimerText { get; private set; } = "25:00";
    [ObservableProperty] public partial FocusStatus? Status { get; private set; }
    [ObservableProperty] public partial string? ErrorMessage { get; private set; }
    [ObservableProperty] public partial int ProgressMaximum { get; private set; } = 1;
    [ObservableProperty] public partial int ProgressValue { get; private set; }
    [ObservableProperty] public partial long TodayFocusedSeconds { get; private set; }
    [ObservableProperty] public partial int TodaySessionCount { get; private set; }

    public bool IsRunning => Status == FocusStatus.Running;
    public bool IsPaused => Status == FocusStatus.Paused;
    public bool IsIdle => Status is null;
    public bool CanConfigureSession => IsIdle;
    public bool IsPomodoroMode => Mode == FocusMode.Pomodoro;
    public bool IsStopwatchMode => Mode == FocusMode.Stopwatch;
    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);
    public string StatusText => Status switch
    {
        FocusStatus.Running => Mode == FocusMode.Pomodoro ? "正在专注" : "计时中",
        FocusStatus.Paused => "已暂停",
        _ => Mode == FocusMode.Pomodoro ? "准备开始一个专注周期" : "准备开始自由计时"
    };
    public string TodaySummaryText => TodaySessionCount == 0
        ? "今天还没有完成专注"
        : $"{FormatDuration(TodayFocusedSeconds)} · {TodaySessionCount} 次专注";

    partial void OnModeChanged(FocusMode value)
    {
        if (IsIdle) ApplyIdleTimer();
        NotifyState();
    }

    partial void OnErrorMessageChanged(string? value) => OnPropertyChanged(nameof(HasError));

    partial void OnSelectedTaskOptionChanged(FocusTaskOption? value)
    {
        OnPropertyChanged(nameof(SelectedTask));
        OnPropertyChanged(nameof(HasSelectedTask));
        OnPropertyChanged(nameof(SelectedTaskTitle));
        OnPropertyChanged(nameof(SelectedTaskMetadata));
    }

    partial void OnSelectedTaskSourceChanged(FocusTaskSourceOption value)
    {
        OnPropertyChanged(nameof(FilteredTaskOptions));
        OnPropertyChanged(nameof(HasFilteredTasks));
        OnPropertyChanged(nameof(IsFilteredTaskListEmpty));
        OnPropertyChanged(nameof(SelectedSourceDescription));
        OnPropertyChanged(nameof(SelectedTaskOption));
    }

    public void SelectTask(long? taskId, bool revealSource = true)
    {
        SelectedTaskOption = taskId is null ? null : TaskOptions.FirstOrDefault(option => option.Task.Id == taskId);
        if (revealSource && SelectedTaskOption is { Task: var task })
        {
            SelectedTaskSource = TaskSources.First(source => source.Source == SourceFor(task));
        }
    }

    public async Task ActivateAsync()
    {
        ErrorMessage = null;
        try
        {
            var current = await sessions.GetCurrentAsync();
            if (current is not null)
            {
                Mode = current.Mode;
                SelectTask(current.TaskId);
                if (current.Mode == FocusMode.Stopwatch) await stopwatch.RestoreAsync();
            }

            Status = current?.Status;
            await RefreshTodaySummaryAsync();
            Refresh();
        }
        catch (Exception exception)
        {
            ErrorMessage = exception.Message;
            NotifyState();
        }
    }

    public void Refresh()
    {
        if (Mode == FocusMode.Stopwatch)
        {
            var snapshot = stopwatch.GetSnapshot();
            Status = snapshot?.Status ?? Status;
            TimerText = FormatTimer(snapshot?.ElapsedSeconds ?? 0);
            ProgressMaximum = 1;
            ProgressValue = 0;
        }
        else if (pomodoro.Current is { } current)
        {
            Status = current.Status;
            var duration = GetPomodoroDurationSeconds(current.PomodoroKind);
            var remaining = Math.Clamp(pomodoro.RemainingSeconds, 0, duration);
            TimerText = FormatTimer(remaining);
            ProgressMaximum = Math.Max(1, duration);
            ProgressValue = Math.Clamp(duration - remaining, 0, ProgressMaximum);
        }
        else if (Status is null)
        {
            ApplyIdleTimer();
        }

        NotifyState();
    }

    public async Task StartAsync()
    {
        ErrorMessage = null;
        try
        {
            Status = Mode == FocusMode.Stopwatch
                ? (await stopwatch.StartAsync(new FocusSessionStartRequest(SelectedTask?.Id, FocusMode.Stopwatch))).Status
                : (await pomodoro.StartAsync(SelectedTask?.Id, PomodoroKind.Focus, settings)).Status;
            Refresh();
        }
        catch (Exception exception)
        {
            ErrorMessage = exception.Message;
            NotifyState();
        }
    }

    public Task PauseAsync() => MutateAsync(async () => Status = Mode == FocusMode.Stopwatch
        ? (await stopwatch.PauseCurrentAsync()).Status
        : (await pomodoro.PauseAsync()).Status);

    public Task ResumeAsync() => MutateAsync(async () => Status = Mode == FocusMode.Stopwatch
        ? (await stopwatch.ResumeCurrentAsync()).Status
        : (await pomodoro.ResumeAsync()).Status);

    public Task StopAsync() => MutateAsync(async () =>
    {
        if (Mode == FocusMode.Stopwatch) await stopwatch.StopCurrentAsync();
        else await pomodoro.StopAsync();
        Status = null;
        await RefreshTodaySummaryAsync();
    });

    public Task CancelAsync() => MutateAsync(async () =>
    {
        if (Mode == FocusMode.Stopwatch) await stopwatch.CancelCurrentAsync();
        else await pomodoro.CancelAsync();
        Status = null;
    });

    private async Task MutateAsync(Func<Task> mutation)
    {
        ErrorMessage = null;
        try
        {
            await mutation();
            Refresh();
        }
        catch (Exception exception)
        {
            ErrorMessage = exception.Message;
            NotifyState();
        }
    }

    private async Task RefreshTodaySummaryAsync()
    {
        var summary = await sessions.GetProductiveSummaryAsync(localDate);
        TodayFocusedSeconds = summary.TotalSeconds;
        TodaySessionCount = summary.SessionCount;
        OnPropertyChanged(nameof(TodaySummaryText));
    }

    private void ApplyIdleTimer()
    {
        var seconds = Mode == FocusMode.Pomodoro ? settings.FocusMinutes * 60 : 0;
        TimerText = FormatTimer(seconds);
        ProgressMaximum = Math.Max(1, seconds);
        ProgressValue = 0;
    }

    private int GetPomodoroDurationSeconds(PomodoroKind? kind) => kind switch
    {
        PomodoroKind.ShortBreak => settings.ShortBreakMinutes * 60,
        PomodoroKind.LongBreak => settings.LongBreakMinutes * 60,
        _ => settings.FocusMinutes * 60
    };

    private void NotifyState()
    {
        OnPropertyChanged(nameof(IsRunning));
        OnPropertyChanged(nameof(IsPaused));
        OnPropertyChanged(nameof(IsIdle));
        OnPropertyChanged(nameof(CanConfigureSession));
        OnPropertyChanged(nameof(IsPomodoroMode));
        OnPropertyChanged(nameof(IsStopwatchMode));
        OnPropertyChanged(nameof(StatusText));
    }

    private static string FormatTaskMetadata(TaskItem task, DateOnly localDate)
    {
        var parts = new List<string>();
        parts.Add(task.QuadrantId is { } quadrantId ? $"Q{quadrantId}" : "Inbox");
        if (task.PlannedDate == localDate) parts.Add("Today");
        if (task.EstimatedMinutes is { } minutes) parts.Add($"预计 {minutes} 分钟");
        return string.Join(" · ", parts);
    }

    private bool IsInSelectedSource(FocusTaskOption option) => SelectedTaskSource.Source switch
    {
        FocusTaskSource.Inbox => option.Task.QuadrantId is null,
        FocusTaskSource.Today => todayTaskIds.Contains(option.Task.Id),
        FocusTaskSource.Quadrant1 => option.Task.QuadrantId == 1,
        FocusTaskSource.Quadrant2 => option.Task.QuadrantId == 2,
        FocusTaskSource.Quadrant3 => option.Task.QuadrantId == 3,
        FocusTaskSource.Quadrant4 => option.Task.QuadrantId == 4,
        _ => false
    };

    private static FocusTaskSource SourceFor(TaskItem task) => task.QuadrantId switch
    {
        1 => FocusTaskSource.Quadrant1,
        2 => FocusTaskSource.Quadrant2,
        3 => FocusTaskSource.Quadrant3,
        4 => FocusTaskSource.Quadrant4,
        _ => FocusTaskSource.Inbox
    };

    private static string FormatTimer(int seconds) => $"{seconds / 60:D2}:{seconds % 60:D2}";

    private static string FormatDuration(long seconds)
    {
        var totalMinutes = seconds / 60;
        var hours = totalMinutes / 60;
        var minutes = totalMinutes % 60;
        return hours > 0 ? $"{hours} 小时 {minutes} 分" : $"{minutes} 分钟";
    }
}

public sealed record FocusTaskOption(TaskItem Task, string Title, string Metadata);

public sealed record FocusTaskSourceOption(FocusTaskSource Source, string Title, string Description);

public enum FocusTaskSource
{
    Inbox = 1,
    Today = 2,
    Quadrant1 = 3,
    Quadrant2 = 4,
    Quadrant3 = 5,
    Quadrant4 = 6
}
