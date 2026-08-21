using System.Drawing;
using Forms = System.Windows.Forms;

namespace Quadrant.Infrastructure.Windows;

public sealed class TrayService : IDisposable
{
    private Forms.NotifyIcon? notifyIcon;
    private bool isDisposed;

    public event EventHandler? ShowRequested;

    public event EventHandler? QuickAddRequested;

    public event EventHandler? ExitRequested;

    public void Initialize(Icon icon)
    {
        ArgumentNullException.ThrowIfNull(icon);
        ObjectDisposedException.ThrowIf(isDisposed, this);

        if (notifyIcon is not null)
        {
            return;
        }

        var menu = new Forms.ContextMenuStrip();
        menu.Items.Add("新建任务", null, (_, _) => QuickAddRequested?.Invoke(this, EventArgs.Empty));
        menu.Items.Add("显示主窗口", null, (_, _) => ShowRequested?.Invoke(this, EventArgs.Empty));
        menu.Items.Add(new Forms.ToolStripSeparator());
        menu.Items.Add("退出", null, (_, _) => ExitRequested?.Invoke(this, EventArgs.Empty));

        notifyIcon = new Forms.NotifyIcon
        {
            Icon = icon,
            Text = "Quadrant",
            ContextMenuStrip = menu,
            Visible = true
        };
        notifyIcon.DoubleClick += NotifyIcon_DoubleClick;
    }

    public void Dispose()
    {
        if (isDisposed)
        {
            return;
        }

        if (notifyIcon is not null)
        {
            notifyIcon.DoubleClick -= NotifyIcon_DoubleClick;
            notifyIcon.Visible = false;
            notifyIcon.Dispose();
            notifyIcon = null;
        }

        isDisposed = true;
    }

    private void NotifyIcon_DoubleClick(object? sender, EventArgs e) => ShowRequested?.Invoke(this, e);
}
