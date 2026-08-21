using Quadrant.Core.Enums;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class FocusTimerService : IFocusTimerService
{
    private readonly IFocusSessionService sessions; private readonly IClock clock;
    private FocusSession? current; private long segmentTimestamp; private int accumulated;
    public FocusTimerService(IFocusSessionService sessions, IClock clock) { this.sessions=sessions; this.clock=clock; }
    public FocusTimerSnapshot? Current => GetSnapshot();
    public async Task<FocusTimerSnapshot> StartAsync(FocusSessionStartRequest request,CancellationToken ct=default)
    { if(current is not null) throw new InvalidOperationException("A focus timer is already active."); current=await sessions.StartAsync(request,ct); accumulated=current.DurationSeconds; segmentTimestamp=clock.GetTimestamp(); return Snapshot(accumulated); }
    public async Task<FocusTimerSnapshot?> RestoreAsync(CancellationToken ct=default)
    { if(current is not null)return GetSnapshot(); current=await sessions.GetCurrentAsync(ct); if(current is null)return null; accumulated=current.DurationSeconds; if(current.Status==FocusStatus.Running){ var downtime=clock.UtcNow-current.ActiveSegmentStartedAtUtc!.Value; accumulated+=Floor(downtime); segmentTimestamp=clock.GetTimestamp(); } return Snapshot(accumulated); }
    public FocusTimerSnapshot? GetSnapshot() => current is null?null:Snapshot(Elapsed());
    public async Task<FocusTimerSnapshot> PauseCurrentAsync(CancellationToken ct=default)
    { var session=Require(FocusStatus.Running); accumulated=Elapsed(); current=await sessions.PauseAsync(session.Id,accumulated,clock.UtcNow,ct); return Snapshot(accumulated); }
    public async Task<FocusTimerSnapshot> ResumeCurrentAsync(CancellationToken ct=default)
    { var session=Require(FocusStatus.Paused); current=await sessions.ResumeAsync(session.Id,clock.UtcNow,cancellationToken:ct); segmentTimestamp=clock.GetTimestamp(); return Snapshot(accumulated); }
    public async Task<FocusSession> StopCurrentAsync(CancellationToken ct=default)
    { var session=Require(FocusStatus.Running,FocusStatus.Paused); var total=Elapsed(); var result=await sessions.CompleteAsync(session.Id,total,clock.UtcNow,ct); current=null;return result; }
    public async Task<FocusSession> CancelCurrentAsync(CancellationToken ct=default)
    { var session=Require(FocusStatus.Running,FocusStatus.Paused); var total=Elapsed(); var result=await sessions.CancelAsync(session.Id,total,clock.UtcNow,ct); current=null;return result; }
    private FocusSession Require(params FocusStatus[] allowed)=>current is not null&&allowed.Contains(current.Status)?current:throw new InvalidOperationException("Focus timer is not in a valid state.");
    private int Elapsed()=>current?.Status==FocusStatus.Running?accumulated+Floor(clock.GetElapsedTime(segmentTimestamp,clock.GetTimestamp())):accumulated;
    private FocusTimerSnapshot Snapshot(int elapsed)=>new(current!.Id,current.Status,elapsed,current);
    private static int Floor(TimeSpan value)=>(int)Math.Max(0,Math.Floor(value.TotalSeconds));
}
