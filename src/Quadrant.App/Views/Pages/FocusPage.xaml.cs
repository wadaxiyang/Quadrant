using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Quadrant.App.ViewModels;

namespace Quadrant.App.Views.Pages;

public partial class FocusPage : Page
{
    private DispatcherTimer? timer;

    public FocusPage() => InitializeComponent();

    private async void Page_Loaded(object sender, RoutedEventArgs e)
    {
        var request = DataContext as FocusPageNavigationRequest;
        var main = request?.MainViewModel ?? DataContext as MainViewModel;
        if (main is not null)
        {
            var focusViewModel = await FocusPageViewModel.CreateAsync(
                main.TaskService,
                main.TodayQueryService,
                main.FocusTimerService,
                main.PomodoroTimerService,
                main.FocusSessionService,
                main.Settings.Pomodoro,
                main.Clock);
            if (request is not null)
            {
                focusViewModel.SelectTask(request.TaskId);
            }

            DataContext = focusViewModel;
        }

        await ViewModel.ActivateAsync();
        UpdateTimerState();
    }

    private void Page_Unloaded(object sender, RoutedEventArgs e) => StopDisplayTimer();

    private async void Start_Click(object sender, RoutedEventArgs e) { await ViewModel.StartAsync(); UpdateTimerState(); }
    private async void Pause_Click(object sender, RoutedEventArgs e) { await ViewModel.PauseAsync(); UpdateTimerState(); }
    private async void Resume_Click(object sender, RoutedEventArgs e) { await ViewModel.ResumeAsync(); UpdateTimerState(); }
    private async void Stop_Click(object sender, RoutedEventArgs e) { await ViewModel.StopAsync(); UpdateTimerState(); }
    private async void Cancel_Click(object sender, RoutedEventArgs e) { await ViewModel.CancelAsync(); UpdateTimerState(); }
    private void ClearTask_Click(object sender, RoutedEventArgs e) => ViewModel.SelectTask(null);
    private void TaskPicker_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (ViewModel.CanConfigureSession && e.AddedItems.OfType<FocusTaskOption>().FirstOrDefault() is { } option)
        {
            ViewModel.SelectTask(option.Task.Id, revealSource: false);
        }
    }
    private void PomodoroMode_Click(object sender, RoutedEventArgs e) { if (ViewModel.CanConfigureSession) ViewModel.Mode = Quadrant.Core.Enums.FocusMode.Pomodoro; }
    private void StopwatchMode_Click(object sender, RoutedEventArgs e) { if (ViewModel.CanConfigureSession) ViewModel.Mode = Quadrant.Core.Enums.FocusMode.Stopwatch; }

    private FocusPageViewModel ViewModel => (FocusPageViewModel)DataContext;

    private void UpdateTimerState()
    {
        if (!IsLoaded || !ViewModel.IsRunning)
        {
            StopDisplayTimer();
            return;
        }

        if (timer is not null) return;
        timer = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromSeconds(1) };
        timer.Tick += DisplayTimer_Tick;
        timer.Start();
    }

    private void DisplayTimer_Tick(object? sender, EventArgs e)
    {
        ViewModel.Refresh();
        UpdateTimerState();
    }

    private void StopDisplayTimer()
    {
        if (timer is null) return;
        timer.Stop();
        timer.Tick -= DisplayTimer_Tick;
        timer = null;
    }
}

public sealed record FocusPageNavigationRequest(MainViewModel MainViewModel, long TaskId);
