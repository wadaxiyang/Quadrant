using System.Windows;

namespace Quadrant.App.Views;

public partial class CompletedWindow : Window
{
    public CompletedWindow(object dataContext)
    {
        InitializeComponent();
        DataContext = dataContext;
    }
}
