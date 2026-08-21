using System.Windows;
using System.Windows.Input;
using Quadrant.App.ViewModels;
using Quadrant.Core.Models;

namespace Quadrant.App.Views;

public partial class QuickAddWindow : Wpf.Ui.Controls.FluentWindow
{
    public QuickAddWindow(TaskEditorViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;
        Loaded += (_, _) =>
        {
            TitleBox.Focus();
            TitleBox.SelectAll();
        };
    }

    public TaskDraft? DraftResult { get; private set; }

    private void Destination_Click(object sender, RoutedEventArgs e)
    {
        if (sender is System.Windows.Controls.Button { Tag: string tag })
        {
            var viewModel = (TaskEditorViewModel)DataContext;
            viewModel.QuadrantId = string.Equals(tag, "Inbox", StringComparison.Ordinal) ? null : int.TryParse(tag, out var id) ? id : viewModel.QuadrantId;
            if (sender is System.Windows.Controls.Primitives.ToggleButton toggleButton)
            {
                // Destination is mandatory: clicking the current choice must not
                // leave Quick Capture with no selected destination.
                toggleButton.IsChecked = true;
            }
        }
    }

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        if (((TaskEditorViewModel)DataContext).TryBuildDraft(out var draft))
        {
            DraftResult = draft;
            DialogResult = true;
        }
    }

    private void Window_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (Keyboard.Modifiers == ModifierKeys.Control && (e.Key is >= Key.D1 and <= Key.D4 || e.Key is >= Key.NumPad1 and <= Key.NumPad4))
        {
            ((TaskEditorViewModel)DataContext).QuadrantId = e.Key is >= Key.NumPad1 and <= Key.NumPad4 ? e.Key - Key.NumPad0 : e.Key - Key.D0;
            e.Handled = true;
        }
    }
}
