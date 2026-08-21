using Quadrant.Core.Interfaces;
namespace Quadrant.Core.Services;
public sealed class SystemFocusCompletionScheduler : IFocusCompletionScheduler
{
 public IDisposable Schedule(DateTimeOffset dueAtUtc, Func<Task> callback)
 { var delay=dueAtUtc-DateTimeOffset.UtcNow;if(delay<TimeSpan.Zero)delay=TimeSpan.Zero;Timer? timer=null;timer=new Timer(async _=>{try{await callback();}finally{timer?.Dispose();}},null,delay,Timeout.InfiniteTimeSpan);return timer; }
}
