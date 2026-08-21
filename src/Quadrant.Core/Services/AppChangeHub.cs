using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class AppChangeHub(IDiagnosticLogger? diagnosticLogger = null) : IAppChangeHub
{
    private readonly object gate = new();
    private readonly List<Action<AppChange>> subscribers = [];

    public IDisposable Subscribe(Action<AppChange> subscriber)
    {
        ArgumentNullException.ThrowIfNull(subscriber);
        lock (gate)
        {
            subscribers.Add(subscriber);
        }

        return new Subscription(this, subscriber);
    }

    public void Publish(AppChange change)
    {
        ArgumentNullException.ThrowIfNull(change);
        Action<AppChange>[] currentSubscribers;
        lock (gate)
        {
            currentSubscribers = subscribers.ToArray();
        }

        foreach (var subscriber in currentSubscribers)
        {
            try
            {
                subscriber(change);
            }
            catch (Exception exception)
            {
                diagnosticLogger?.Warning($"App change subscriber failed for {change.Kind} task {change.TaskId}.", exception);
            }
        }
    }

    private void Unsubscribe(Action<AppChange> subscriber)
    {
        lock (gate)
        {
            subscribers.Remove(subscriber);
        }
    }

    private sealed class Subscription(AppChangeHub owner, Action<AppChange> subscriber) : IDisposable
    {
        private AppChangeHub? owner = owner;

        public void Dispose()
        {
            var currentOwner = Interlocked.Exchange(ref owner, null);
            currentOwner?.Unsubscribe(subscriber);
        }
    }
}
