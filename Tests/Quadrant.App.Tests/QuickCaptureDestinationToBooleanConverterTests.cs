using System.Globalization;
using Quadrant.App.Converters;
using Xunit;

namespace Quadrant.App.Tests;

public sealed class QuickCaptureDestinationToBooleanConverterTests
{
    [Fact]
    public void Checked_destination_maps_to_exactly_one_nullable_quadrant_value()
    {
        var converter = new QuickCaptureDestinationToBooleanConverter();

        Assert.Null(converter.ConvertBack(true, typeof(int?), "Inbox", CultureInfo.InvariantCulture));
        Assert.Equal(3, converter.ConvertBack(true, typeof(int?), "3", CultureInfo.InvariantCulture));
        Assert.Same(System.Windows.Data.Binding.DoNothing, converter.ConvertBack(false, typeof(int?), "3", CultureInfo.InvariantCulture));
    }
}
