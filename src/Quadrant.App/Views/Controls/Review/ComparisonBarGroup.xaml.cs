using System.Windows;
using System.Windows.Controls;

namespace Quadrant.App.Views.Controls.Review;

public partial class ComparisonBarGroup : System.Windows.Controls.UserControl
{
    public static readonly DependencyProperty MetricNameProperty = Register(nameof(MetricName), typeof(string), string.Empty);
    public static readonly DependencyProperty CurrentLabelProperty = Register(nameof(CurrentLabel), typeof(string), string.Empty);
    public static readonly DependencyProperty PreviousLabelProperty = Register(nameof(PreviousLabel), typeof(string), string.Empty);
    public static readonly DependencyProperty CurrentValueProperty = Register(nameof(CurrentValue), typeof(double), 0d);
    public static readonly DependencyProperty PreviousValueProperty = Register(nameof(PreviousValue), typeof(double), 0d);
    public static readonly DependencyProperty CurrentTextProperty = Register(nameof(CurrentText), typeof(string), string.Empty);
    public static readonly DependencyProperty PreviousTextProperty = Register(nameof(PreviousText), typeof(string), string.Empty);
    public static readonly DependencyProperty DeltaTextProperty = Register(nameof(DeltaText), typeof(string), string.Empty);

    public ComparisonBarGroup() => InitializeComponent();
    public string MetricName { get => (string)GetValue(MetricNameProperty); set => SetValue(MetricNameProperty, value); }
    public string CurrentLabel { get => (string)GetValue(CurrentLabelProperty); set => SetValue(CurrentLabelProperty, value); }
    public string PreviousLabel { get => (string)GetValue(PreviousLabelProperty); set => SetValue(PreviousLabelProperty, value); }
    public double CurrentValue { get => (double)GetValue(CurrentValueProperty); set => SetValue(CurrentValueProperty, value); }
    public double PreviousValue { get => (double)GetValue(PreviousValueProperty); set => SetValue(PreviousValueProperty, value); }
    public string CurrentText { get => (string)GetValue(CurrentTextProperty); set => SetValue(CurrentTextProperty, value); }
    public string PreviousText { get => (string)GetValue(PreviousTextProperty); set => SetValue(PreviousTextProperty, value); }
    public string DeltaText { get => (string)GetValue(DeltaTextProperty); set => SetValue(DeltaTextProperty, value); }
    private static DependencyProperty Register(string name, Type type, object value) => DependencyProperty.Register(name, type, typeof(ComparisonBarGroup), new PropertyMetadata(value));
}
