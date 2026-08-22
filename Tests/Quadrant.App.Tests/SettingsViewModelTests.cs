using Quadrant.App.ViewModels;
using Quadrant.Core.Enums;
using Quadrant.Core.Models;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class SettingsViewModelTests
{
    private static readonly QuadrantDefinition[] Quadrants =
    [new(1, "Q1", "A"), new(2, "Q2", "B"), new(3, "Q3", "C"), new(4, "Q4", "D")];

    [Fact]
    public void V2_settings_are_validated_and_built_as_one_snapshot()
    {
        var viewModel = new SettingsViewModel(AppSettings.Default, Quadrants)
        {
            QuickCaptureQuadrantId = 2,
            DefaultReminder = ReminderPreset.OneHourBefore,
            FocusMinutes = 50,
            AutoStartBreak = true,
            ReviewDefaultRange = ReviewRange.ThirtyDays,
            WeekStart = DayOfWeek.Sunday,
            SidebarIconSize = 27
        };

        var settings = viewModel.BuildSettings();

        Assert.Equal(2, settings.QuickCaptureQuadrantId);
        Assert.Equal(ReminderPreset.OneHourBefore, settings.DefaultReminder);
        Assert.Equal(50, settings.Pomodoro.FocusMinutes);
        Assert.True(settings.Pomodoro.AutoStartBreak);
        Assert.Equal(ReviewRange.ThirtyDays, settings.ReviewDefaultRange);
        Assert.Equal(27, settings.SidebarIconSize);
    }

    [Theory]
    [InlineData(0, 5, 15, 4)]
    [InlineData(25, 0, 15, 4)]
    [InlineData(25, 5, 121, 4)]
    [InlineData(25, 5, 15, 13)]
    public void Invalid_focus_boundaries_prevent_partial_save(int focus, int shortBreak, int longBreak, int interval)
    {
        var viewModel = new SettingsViewModel(AppSettings.Default, Quadrants)
        {
            FocusMinutes = focus,
            ShortBreakMinutes = shortBreak,
            LongBreakMinutes = longBreak,
            LongBreakInterval = interval
        };

        Assert.ThrowsAny<Exception>(() => viewModel.BuildSettings());
    }
}
