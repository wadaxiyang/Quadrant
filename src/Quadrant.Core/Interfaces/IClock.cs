namespace Quadrant.Core.Interfaces;

public interface IClock
{
    DateTimeOffset Now { get; }
}
