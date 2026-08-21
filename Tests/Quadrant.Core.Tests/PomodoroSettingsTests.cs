using Quadrant.Core.Models; using Quadrant.Core.Services; using Xunit;
namespace Quadrant.Core.Tests;
public sealed class PomodoroSettingsTests { [Fact] public void Settings_validate_bounds(){new PomodoroSettings().Validate();Assert.Throws<TaskValidationException>(()=>new PomodoroSettings(0).Validate());Assert.Throws<TaskValidationException>(()=>new PomodoroSettings(LongBreakInterval:13).Validate());} }
