using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class TodayQueryService : ITodayQueryService
{
    private readonly ITodayTaskRepository repository;
    private readonly IFocusSessionRepository focusSessionRepository;
    private readonly IClock clock;

    public TodayQueryService(
        ITodayTaskRepository repository,
        IFocusSessionRepository focusSessionRepository,
        IClock clock)
    {
        this.repository = repository ?? throw new ArgumentNullException(nameof(repository));
        this.focusSessionRepository = focusSessionRepository ?? throw new ArgumentNullException(nameof(focusSessionRepository));
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
    }

    public async Task<TodaySnapshot> GetSnapshotAsync(CancellationToken cancellationToken = default)
    {
        var now = clock.LocalNow;
        var timeZone = clock.LocalTimeZone;
        var localToday = DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(now, timeZone).Date);
        var candidatesTask = repository.GetTodayCandidatesAsync(localToday, cancellationToken);
        var focusSummaryTask = focusSessionRepository.GetProductiveSummaryAsync(localToday, cancellationToken);
        await Task.WhenAll(candidatesTask, focusSummaryTask);
        var candidates = await candidatesTask;
        var focusSummary = await focusSummaryTask;
        var assigned = new HashSet<long>();

        var overdue = Assign(candidates.Where(task => IsOverdue(task, now)), assigned);
        var plannedToday = Assign(candidates.Where(task => task.PlannedDate == localToday), assigned);
        var dueToday = Assign(candidates.Where(task => IsDueOn(task, localToday, timeZone)), assigned);
        var needsReschedule = Assign(candidates.Where(task => task.PlannedDate < localToday), assigned);
        var all = overdue.Concat(plannedToday).Concat(dueToday).Concat(needsReschedule).ToArray();

        return new TodaySnapshot(
            overdue,
            plannedToday,
            dueToday,
            needsReschedule,
            all.Length,
            all.Aggregate(0L, (total, task) => checked(total + (task.EstimatedMinutes ?? 0))),
            FocusedSecondsToday: focusSummary.TotalSeconds);
    }

    private static IReadOnlyList<TaskItem> Assign(IEnumerable<TaskItem> source, HashSet<long> assigned) =>
        source.Where(task => !task.IsCompleted && assigned.Add(task.Id))
            .OrderBy(task => task.DueAt is null)
            .ThenBy(task => task.DueAt)
            .ThenBy(task => QuadrantSortKey(task.QuadrantId))
            .ThenBy(task => task.CreatedAt)
            .ThenBy(task => task.Id)
            .ToArray();

    private static bool IsOverdue(TaskItem task, DateTimeOffset now) => task.DueAt is { } due && due < now;

    private static bool IsDueOn(TaskItem task, DateOnly localToday, TimeZoneInfo timeZone) =>
        task.DueAt is { } due && DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(due, timeZone).Date) == localToday;

    private static int QuadrantSortKey(int? quadrantId) => quadrantId is >= 1 and <= 4 ? quadrantId.Value : 5;
}
