namespace Quadrant.Core.Interfaces;
public interface IFocusCompletionScheduler { IDisposable Schedule(DateTimeOffset dueAtUtc, Func<Task> callback); }
