using Quadrant.Core.Enums;

namespace Quadrant.Core.Models;

public sealed record AppChange(long TaskId, AppChangeKind Kind);
