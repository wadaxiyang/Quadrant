using System.Windows;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;

namespace Quadrant.App.Views;

public partial class TaskEditorWindow : Wpf.Ui.Controls.FluentWindow
{
    public TaskEditorWindow(TaskEditorViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;
        Loaded += (_, _) =>
        {
            MaxHeight = SystemParameters.WorkArea.Height;
            MaxWidth = SystemParameters.WorkArea.Width;
            TitleBox.Focus();
        };
    }

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        var viewModel = (TaskEditorViewModel)DataContext;
        if (viewModel.IsEdit)
        {
            if (viewModel.TryBuildUpdate(out var update))
            {
                UpdateResult = update;
                DialogResult = true;
            }
        }
        else if (viewModel.TryBuildDraft(out var draft))
        {
            DraftResult = draft;
            DialogResult = true;
        }
    }

    public TaskDraft? DraftResult { get; private set; }

    public TaskUpdate? UpdateResult { get; private set; }
}
