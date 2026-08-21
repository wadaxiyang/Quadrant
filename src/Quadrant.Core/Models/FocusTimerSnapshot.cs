using Quadrant.Core.Enums;

namespace Quadrant.Core.Models;

public sealed record FocusTimerSnapshot(string SessionId, FocusStatus Status, int ElapsedSeconds, FocusSession Session);
