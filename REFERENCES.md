# Official References — Verified during planning on 2026-08-20

> 这些链接是设计基线，不替代执行 Stage 时的重新联网核验。

## WPF / .NET 10

1. What's new in WPF for .NET 10  
   https://learn.microsoft.com/en-us/dotnet/desktop/wpf/whats-new/net100

   规划时确认：.NET 10 对 WPF 做性能改进，并继续补充/修复 Fluent styles；DatePicker、TextBox 等更多控件已覆盖。

2. What's new in WPF  
   https://learn.microsoft.com/en-us/dotnet/desktop/wpf/whats-new/

3. What's new in WPF for .NET 9 — Fluent Theme / ThemeMode  
   https://learn.microsoft.com/en-us/dotnet/desktop/wpf/whats-new/net90

4. WPF styles and templates — modern Fluent theme (.NET 9+)  
   https://learn.microsoft.com/en-us/dotnet/desktop/wpf/controls/styles-and-templates

## MVVM

5. MVVM Toolkit introduction  
   https://learn.microsoft.com/en-us/dotnet/communitytoolkit/mvvm/

6. MVVM source generators  
   https://learn.microsoft.com/en-us/dotnet/communitytoolkit/mvvm/generators/overview

7. ObservableProperty  
   https://learn.microsoft.com/en-us/dotnet/communitytoolkit/mvvm/generators/observableproperty

8. RelayCommand  
   https://learn.microsoft.com/en-us/dotnet/communitytoolkit/mvvm/generators/relaycommand

9. CommunityToolkit.Mvvm NuGet  
   https://www.nuget.org/packages/CommunityToolkit.Mvvm/

   规划时最新稳定：**8.4.2**（2026-03-25）。执行 Stage 00 时重新确认。

## SQLite

10. Microsoft.Data.Sqlite transactions  
    https://learn.microsoft.com/en-us/dotnet/standard/data/sqlite/transactions

11. Microsoft.Data.Sqlite NuGet  
    https://www.nuget.org/packages/Microsoft.Data.Sqlite/

    规划时 .NET 10 stable line 最新：**10.0.11**（2026-08-11）。执行 Stage 00 时重新确认。

## Windows App Notifications

12. Use app notifications with a .NET app (WPF/WinForms)  
    https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-dotnet

    规划时确认：WPF .NET 6+ 可使用 `Microsoft.Windows.AppNotifications`；unpackaged app 调用 Register 时可自动设置通知 activation 所需 COM 注册。

13. Schedule an app notification  
    https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-scheduled

    规划时确认：Scheduled notification 可以在 app 未运行时显示；delivery window 约 **5 minutes**。

14. AppNotificationBuilder.AddButton  
    https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.windows.appnotifications.builder.appnotificationbuilder.addbutton

15. ScheduledToastNotification.Id  
    https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.scheduledtoastnotification.id

    规划时确认：ID developer-specified，当前文档限制 **16 chars**。

16. Latest Windows App SDK downloads  
    https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/downloads

    规划时 stable runtime release line 最新显示：**2.3.1 (2026-07-16)**。实际 NuGet package version 必须在 Stage 10 查官方 release/nuget 后固定。

## Windows App SDK Deployment

17. Deployment overview  
    https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/deploy-overview

18. Framework-dependent unpackaged deployment  
    https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/deploy-unpackaged-apps

19. Self-contained deployment  
    https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/self-contained-deploy/deploy-self-contained-apps

20. Use Windows App SDK runtime for unpackaged apps  
    https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/use-windows-app-sdk-run-time

## Win32 Hotkey / Tray / Startup

21. RegisterHotKey  
    https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey

    规划时确认：系统级热键通过 `WM_HOTKEY`；支持 `MOD_NOREPEAT`；Win-key 组合通常保留给系统；F12 保留给调试器。

22. UnregisterHotKey  
    https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-unregisterhotkey

23. NotifyIcon (.NET Windows Forms)  
    https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.notifyicon

24. Startup apps  
    https://learn.microsoft.com/en-us/windows/win32/w8cookbook/startup-apps

    规划时确认：Windows startup apps 包括 Run registry keys 与 Startup folders；Task Manager 可让用户禁用。

## .NET Deployment

25. Single-file deployment  
    https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview

V1 不以“单 exe”作为强约束；通知/Windows App SDK 正确部署优先。

## Single instance / app lifecycle

26. Multi-instance apps with Windows App SDK  
    https://learn.microsoft.com/en-us/windows/apps/develop/launch/multi-instance-apps

27. Handle activation with a WPF/.NET app — includes single-instance redirection pattern  
    https://learn.microsoft.com/en-us/windows/apps/develop/launch/handle-uri-activation-dotnet

    规划时确认：Windows App SDK `AppInstance.FindOrRegisterForKey` 与 `RedirectActivationToAsync` 可用于把后续激活重定向到已运行实例；WPF 示例特别提示避免在 WPF SynchronizationContext 上造成重定向死锁。
