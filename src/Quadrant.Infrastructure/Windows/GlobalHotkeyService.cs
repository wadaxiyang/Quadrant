using System.ComponentModel;
using System.Runtime.InteropServices;

namespace Quadrant.Infrastructure.Windows;

public sealed class GlobalHotkeyService : IDisposable
{
    private const int HotkeyId = 0x514;
    private const uint ModAlt = 0x0001;
    private const uint ModControl = 0x0002;
    private const uint ModNoRepeat = 0x4000;
    private const uint VirtualKeyQ = 0x51;
    private const int WmHotkey = 0x0312;

    private bool isRegistered;
    private IntPtr registeredHandle;
    private bool isDisposed;

    public event EventHandler<GlobalHotkeyRegistrationFailedEventArgs>? RegistrationFailed;

    public bool Register(IntPtr handle)
    {
        ObjectDisposedException.ThrowIf(isDisposed, this);

        Unregister(handle);

        isRegistered = RegisterHotKey(handle, HotkeyId, ModControl | ModAlt | ModNoRepeat, VirtualKeyQ);
        registeredHandle = isRegistered ? handle : IntPtr.Zero;
        if (!isRegistered)
        {
            var error = Marshal.GetLastWin32Error();
            RegistrationFailed?.Invoke(this, new GlobalHotkeyRegistrationFailedEventArgs(error));
        }

        return isRegistered;
    }

    public void Unregister(IntPtr handle)
    {
        if (isRegistered)
        {
            UnregisterHotKey(handle, HotkeyId);
            isRegistered = false;
            registeredHandle = IntPtr.Zero;
        }
    }

    public bool IsHotkeyMessage(int message, IntPtr wParam) => message == WmHotkey && wParam.ToInt32() == HotkeyId;

    public void Dispose()
    {
        if (isDisposed)
        {
            return;
        }

        if (isRegistered)
        {
            Unregister(registeredHandle);
        }

        isDisposed = true;
    }

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool UnregisterHotKey(IntPtr hWnd, int id);
}

public sealed class GlobalHotkeyRegistrationFailedEventArgs : EventArgs
{
    public GlobalHotkeyRegistrationFailedEventArgs(int errorCode)
    {
        ErrorCode = errorCode;
    }

    public int ErrorCode { get; }

    public string Message => new Win32Exception(ErrorCode).Message;
}
