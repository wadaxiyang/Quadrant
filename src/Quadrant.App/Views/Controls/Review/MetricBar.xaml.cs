using System.Windows;
using System.Windows.Controls;

namespace Quadrant.App.Views.Controls.Review;

public partial class MetricBar : System.Windows.Controls.UserControl
{
    public static readonly DependencyProperty LabelProperty = DependencyProperty.Register(nameof(Label), typeof(string), typeof(MetricBar), new PropertyMetadata(string.Empty));
    public static readonly DependencyProperty SecondaryLabelProperty = DependencyProperty.Register(nameof(SecondaryLabel), typeof(string), typeof(MetricBar), new PropertyMetadata(string.Empty));
    public static readonly DependencyProperty ValueTextProperty = DependencyProperty.Register(nameof(ValueText), typeof(string), typeof(MetricBar), new PropertyMetadata(string.Empty));
    public static readonly DependencyProperty PercentageTextProperty = DependencyProperty.Register(nameof(PercentageText), typeof(string), typeof(MetricBar), new PropertyMetadata(string.Empty));
    public static readonly DependencyProperty ProgressValueProperty = DependencyProperty.Register(nameof(ProgressValue), typeof(double), typeof(MetricBar), new PropertyMetadata(0d));
    public static readonly DependencyProperty QuadrantIdProperty = DependencyProperty.Register(nameof(QuadrantId), typeof(int?), typeof(MetricBar), new PropertyMetadata(null));

    public MetricBar() => InitializeComponent();
    public string Label { get => (string)GetValue(LabelProperty); set => SetValue(LabelProperty, value); }
    public string SecondaryLabel { get => (string)GetValue(SecondaryLabelProperty); set => SetValue(SecondaryLabelProperty, value); }
    public string ValueText { get => (string)GetValue(ValueTextProperty); set => SetValue(ValueTextProperty, value); }
    public string PercentageText { get => (string)GetValue(PercentageTextProperty); set => SetValue(PercentageTextProperty, value); }
    public double ProgressValue { get => (double)GetValue(ProgressValueProperty); set => SetValue(ProgressValueProperty, value); }
    public int? QuadrantId { get => (int?)GetValue(QuadrantIdProperty); set => SetValue(QuadrantIdProperty, value); }
}
