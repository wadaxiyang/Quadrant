namespace Quadrant.Core.Models;

public sealed record CompletedTaskMutationResult(TaskItem Task, CompletionEvent? Event, bool WasAlreadyCompleted, TaskItem? NextTask = null);
