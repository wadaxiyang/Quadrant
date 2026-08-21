using Microsoft.Windows.AppLifecycle;

namespace Quadrant.Infrastructure.Windows;

public sealed class SingleInstanceService : IDisposable
{
    private AppInstance? instance;

    public bool RegisterCurrentInstance(EventHandler<AppActivationArguments> activationHandler)
    {
        ArgumentNullException.ThrowIfNull(activationHandler);

        instance = AppInstance.FindOrRegisterForKey("main");
        if (!instance.IsCurrent)
        {
            return false;
        }

        instance.Activated += activationHandler;
        return true;
    }

    public Task RedirectActivationAsync(AppActivationArguments arguments) =>
        instance is null
            ? throw new InvalidOperationException("The single instance has not been registered.")
            : instance.RedirectActivationToAsync(arguments).AsTask();

    public void Dispose()
    {
        instance = null;
    }
}
