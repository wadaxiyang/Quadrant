using System.Windows;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;

namespace Quadrant.App.Views;

public partial class TaskEditorWindow : Wpf.Ui.Controls.FluentWindow
{
    private readonly bool focusRecurrence;

    public TaskEditorWindow(TaskEditorViewModel viewModel, bool focusRecurrence = false)
    {
        InitializeComponent();
        DataContext = viewModel;
        this.focusRecurrence = focusRecurrence;
        Loaded += (_, _) =>
        {
            MaxHeight = SystemParameters.WorkArea.Height;
            MaxWidth = SystemParameters.WorkArea.Width;
            if (this.focusRecurrence)
            {
                RecurrenceCombo.Focus();
            }
            else
            {
                TitleBox.Focus();
            }
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
