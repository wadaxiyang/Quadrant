namespace Quadrant.Core.Services;

public sealed class TaskValidationException : ArgumentException
{
    public TaskValidationException(string message)
        : base(message)
    {
    }
}
