using Quadrant.Core.Models;

namespace Quadrant.Core.Interfaces;

public interface IAppChangeHub
{
    IDisposable Subscribe(Action<AppChange> subscriber);

    void Publish(AppChange change);
}
