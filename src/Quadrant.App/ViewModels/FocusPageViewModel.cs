using CommunityToolkit.Mvvm.ComponentModel; using Quadrant.Core.Enums; using Quadrant.Core.Interfaces; using Quadrant.Core.Models; using Quadrant.Core.Services;
namespace Quadrant.App.ViewModels;
public partial class FocusPageViewModel:ObservableObject
{
 private readonly IFocusTimerService stopwatch; private readonly PomodoroTimerService pomodoro; private readonly IFocusSessionService sessions; private readonly PomodoroSettings settings;
 public FocusPageViewModel(IReadOnlyList<TaskItem> tasks,IFocusTimerService stopwatch,PomodoroTimerService pomodoro,IFocusSessionService sessions,PomodoroSettings? settings=null){Tasks=tasks.Where(t=>!t.IsCompleted&&t.QuadrantId is not null).ToArray();this.stopwatch=stopwatch;this.pomodoro=pomodoro;this.sessions=sessions;this.settings=settings??new PomodoroSettings();TimerText=$"{this.settings.FocusMinutes:D2}:00";}
 public IReadOnlyList<TaskItem> Tasks{get;} [ObservableProperty] public partial TaskItem? SelectedTask{get;set;} [ObservableProperty] public partial FocusMode Mode{get;set;}=FocusMode.Pomodoro; [ObservableProperty] public partial string TimerText{get;set;}="25:00"; [ObservableProperty] public partial FocusStatus? Status{get;set;} [ObservableProperty] public partial string? ErrorMessage{get;set;}
 public IReadOnlyList<FocusMode> Modes=>Enum.GetValues<FocusMode>();
 public bool IsRunning=>Status==FocusStatus.Running; public bool IsPaused=>Status==FocusStatus.Paused; public bool IsIdle=>Status is null;
 public async Task ActivateAsync(){var s=await sessions.GetCurrentAsync();if(s is not null)Mode=s.Mode;Status=s?.Status;Refresh();}
 public void Refresh(){var snap=Mode==FocusMode.Stopwatch?stopwatch.GetSnapshot():null;var secs=snap?.ElapsedSeconds??pomodoro.RemainingSeconds;TimerText=Mode==FocusMode.Stopwatch?$"{secs/60:D2}:{secs%60:D2}":$"{secs/60:D2}:{secs%60:D2}";Status=snap?.Status??pomodoro.Current?.Status??Status;Notify();}
 public async Task StartAsync(){try{if(Mode==FocusMode.Stopwatch){var s=await stopwatch.StartAsync(new FocusSessionStartRequest(SelectedTask?.Id,FocusMode.Stopwatch));Status=s.Status;}else{var s=await pomodoro.StartAsync(SelectedTask?.Id,PomodoroKind.Focus,settings);Status=s.Status;}Refresh();}catch(Exception e){ErrorMessage=e.Message;}}
 public async Task PauseAsync(){try{if(Mode==FocusMode.Stopwatch)Status=(await stopwatch.PauseCurrentAsync()).Status;else Status=(await pomodoro.PauseAsync()).Status;Refresh();}catch(Exception e){ErrorMessage=e.Message;}}
 public async Task ResumeAsync(){try{if(Mode==FocusMode.Stopwatch)Status=(await stopwatch.ResumeCurrentAsync()).Status;else Status=(await pomodoro.ResumeAsync()).Status;Refresh();}catch(Exception e){ErrorMessage=e.Message;}}
 public async Task StopAsync(){try{if(Mode==FocusMode.Stopwatch)await stopwatch.StopCurrentAsync();else await pomodoro.StopAsync();Status=null;Refresh();}catch(Exception e){ErrorMessage=e.Message;}}
 public async Task CancelAsync(){try{if(Mode==FocusMode.Stopwatch)await stopwatch.CancelCurrentAsync();else await pomodoro.CancelAsync();Status=null;Refresh();}catch(Exception e){ErrorMessage=e.Message;}}
 private void Notify(){OnPropertyChanged(nameof(IsRunning));OnPropertyChanged(nameof(IsPaused));OnPropertyChanged(nameof(IsIdle));}
}
