using System.Windows;
using Microsoft.Win32;
using Quadrant.App.ViewModels;
using Wpf.Ui.Controls;

namespace Quadrant.App.Views;

public partial class SettingsWindow : Wpf.Ui.Controls.FluentWindow
{
    public SettingsWindow(SettingsViewModel viewModel) { InitializeComponent(); DataContext = viewModel; }
    public SettingsViewModel Settings => (SettingsViewModel)DataContext;
    public Quadrant.Core.Models.AppSettings? DesiredSettings { get; private set; }
    public IReadOnlyList<Quadrant.Core.Models.QuadrantDefinition>? DesiredQuadrants { get; private set; }
    public bool ResetPerformed { get; private set; }

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            DesiredSettings = Settings.BuildSettings();
            DesiredQuadrants = Settings.BuildQuadrants();
            DialogResult = true;
        }
        catch (Exception exception)
        {
            ValidationInfo.Message = exception.Message;
            ValidationInfo.IsOpen = true;
        }
    }

    private async void Backup_Click(object sender, RoutedEventArgs e)
    {
        var dialog = new Microsoft.Win32.SaveFileDialog { Title = "备份 Quadrant 数据库", Filter = "SQLite 数据库 (*.db)|*.db", DefaultExt = ".db", AddExtension = true, FileName = $"Quadrant-backup-{DateTime.Now:yyyyMMdd-HHmm}.db" };
        if (dialog.ShowDialog(this) == true) await RunDataActionAsync(() => Settings.BackupAsync(dialog.FileName), $"备份已保存到：{dialog.FileName}");
    }

    private async void Export_Click(object sender, RoutedEventArgs e)
    {
        var dialog = new Microsoft.Win32.SaveFileDialog { Title = "导出 Quadrant JSON", Filter = "JSON 文件 (*.json)|*.json", DefaultExt = ".json", AddExtension = true, FileName = $"Quadrant-export-{DateTime.Now:yyyyMMdd-HHmm}.json" };
        if (dialog.ShowDialog(this) == true) await RunDataActionAsync(() => Settings.ExportJsonAsync(dialog.FileName), $"JSON 已导出到：{dialog.FileName}");
    }

    private async void ClearFocus_Click(object sender, RoutedEventArgs e)
    {
        if (await ConfirmAsync("清除 Focus 历史？", "这会永久删除全部 Focus session，Review 的专注统计也会清空。建议先备份。", "清除 Focus 历史"))
            await RunDataActionAsync(() => Settings.ClearFocusHistoryAsync(), "Focus 历史已清除。");
    }

    private async void ClearCompletion_Click(object sender, RoutedEventArgs e)
    {
        if (await ConfirmAsync("清除完成历史？", "这会永久删除 Review 使用的完成事件；已完成任务本身会保留。建议先备份。", "清除完成历史"))
            await RunDataActionAsync(() => Settings.ClearCompletionHistoryAsync(), "完成历史已清除。");
    }

    private async void ResetAll_Click(object sender, RoutedEventArgs e)
    {
        if (!await ConfirmAsync("重置全部数据？", "这会永久删除所有任务、Focus 与完成历史，并恢复默认象限和设置。无法撤销，强烈建议先备份。", "永久重置")) return;
        var succeeded = await RunDataActionAsync(() => Settings.ResetAllAsync(), "全部数据已重置。", closeOnSuccess: true);
        if (succeeded) { ResetPerformed = true; DialogResult = false; }
    }

    private async Task<bool> ConfirmAsync(string title, string message, string primaryText)
    {
        var dialog = new ContentDialog(DialogHost) { Title = title, Content = new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap }, PrimaryButtonText = primaryText, CloseButtonText = "取消", PrimaryButtonAppearance = ControlAppearance.Danger, DefaultButton = ContentDialogButton.Close };
        return await dialog.ShowAsync() == ContentDialogResult.Primary;
    }

    private async Task<bool> RunDataActionAsync(Func<Task> action, string success, bool closeOnSuccess = false)
    {
        DataActionButtons.IsEnabled = false;
        DataInfo.IsOpen = true;
        DataInfo.Severity = InfoBarSeverity.Informational;
        DataInfo.Message = "正在处理，请勿关闭窗口。";
        try
        {
            await action();
            DataInfo.Severity = InfoBarSeverity.Success;
            DataInfo.Message = success;
            return true;
        }
        catch (Exception exception)
        {
            DataInfo.Severity = InfoBarSeverity.Error;
            DataInfo.Message = exception.Message;
            return false;
        }
        finally
        {
            if (!closeOnSuccess) DataActionButtons.IsEnabled = true;
        }
    }
}
