using System.Windows;
using System.Windows.Controls;
using Quadrant.App.ViewModels;

namespace Quadrant.App.Views.Pages;

public partial class ReviewPage : Page
{
    private ReviewPageViewModel? viewModel;
    public ReviewPage() => InitializeComponent();
    private async void Page_Loaded(object sender, RoutedEventArgs e)
    {
        if (viewModel is null && DataContext is MainViewModel main)
        {
            viewModel = new ReviewPageViewModel(main.ReviewQueryService ?? throw new InvalidOperationException("Review query service is unavailable."), main.AppChangeHub);
            DataContext = viewModel;
        }
        if (viewModel is not null) await viewModel.ActivateAsync();
    }
    private void Page_Unloaded(object sender, RoutedEventArgs e) => viewModel?.Deactivate();
    private async void Retry_Click(object sender, RoutedEventArgs e) { if (viewModel is not null) await viewModel.LoadAsync(); }
}
