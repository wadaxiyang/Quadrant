using System.Windows;

namespace Quadrant.App.Views;

public partial class CompletedWindow : Wpf.Ui.Controls.FluentWindow
{
    public CompletedWindow(object dataContext)
    {
        InitializeComponent();
        DataContext = dataContext;
    }
}
