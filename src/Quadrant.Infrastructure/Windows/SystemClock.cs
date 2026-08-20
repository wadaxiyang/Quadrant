using Quadrant.Core.Interfaces;

namespace Quadrant.Infrastructure.Windows;

public sealed class SystemClock : IClock
{
    public DateTimeOffset Now => DateTimeOffset.Now;
}
