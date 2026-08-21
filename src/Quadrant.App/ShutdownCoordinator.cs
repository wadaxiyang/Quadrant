namespace Quadrant.App;

internal sealed class ShutdownCoordinator
{
    private bool isExiting;

    public bool IsExiting => isExiting;

    public void BeginExit()
    {
        isExiting = true;
    }
}
