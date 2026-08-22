using System.Globalization;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Core.Services;

public sealed class ReviewInsightService : IReviewInsightService
{
    public IReadOnlyList<ReviewInsight> CreateInsights(ReviewDashboard dashboard, int maximum = 5)
    {
        if (maximum is < 1 or > 5) throw new ArgumentOutOfRangeException(nameof(maximum));
        var insights = new List<ReviewInsight>(maximum);

        if (dashboard.Previous is { } previous && dashboard.Current.CompletedTaskCount != previous.CompletedTaskCount)
        {
            var difference = dashboard.Current.CompletedTaskCount - previous.CompletedTaskCount;
            Add(ReviewInsightKind.CompletionChange,
                $"本期完成任务比上期{(difference > 0 ? "多" : "少")} {Math.Abs(difference)} 项。",
                difference > 0 ? ReviewInsightTone.Positive : ReviewInsightTone.Attention);
        }

        var classified = dashboard.CompletedByQuadrant.Where(value => value.QuadrantId is not null).Sum(value => value.Value);
        var important = dashboard.CompletedByQuadrant.Where(value => value.QuadrantId is 1 or 2).Sum(value => value.Value);
        if (classified > 0 && important > 0)
        {
            Add(ReviewInsightKind.ImportantWorkShare,
                $"Q1 和 Q2 占已分类完成任务的 {important / (double)classified:P0}。",
                ReviewInsightTone.Neutral);
        }

        var highestCompletion = UniqueMaximum(dashboard.CompletedByQuadrant.Where(value => value.QuadrantId is not null));
        if (highestCompletion is not null)
            Add(ReviewInsightKind.HighestCompletionQuadrant, $"{highestCompletion.LabelKey} 完成任务最多：{highestCompletion.Value} 项。", ReviewInsightTone.Neutral);

        var highestFocus = UniqueMaximum(dashboard.FocusByQuadrant.Where(value => value.QuadrantId is not null));
        if (highestFocus is not null)
            Add(ReviewInsightKind.HighestFocusQuadrant, $"{highestFocus.LabelKey} 获得最多专注时间：{FormatDuration(highestFocus.Value)}。", ReviewInsightTone.Neutral);

        if (insights.Count < maximum && dashboard.Range is ReviewRange.SevenDays or ReviewRange.ThirtyDays)
        {
            var activeDay = UniqueMaximum(dashboard.CompletedActivity);
            if (activeDay is not null)
                Add(ReviewInsightKind.MostActiveDay, $"{activeDay.StartDate.ToString("M月d日", CultureInfo.CurrentCulture)} 最活跃，完成 {activeDay.Value} 项任务。", ReviewInsightTone.Neutral);
        }

        if (dashboard.Previous is { } previousFocus && dashboard.Current.TotalFocusSeconds != previousFocus.TotalFocusSeconds)
        {
            var difference = dashboard.Current.TotalFocusSeconds - previousFocus.TotalFocusSeconds;
            Add(ReviewInsightKind.FocusChange,
                $"专注时间比上期{(difference > 0 ? "增加" : "减少")} {FormatDuration(Math.Abs(difference))}。",
                difference > 0 ? ReviewInsightTone.Positive : ReviewInsightTone.Attention);
        }

        if (dashboard.Current.HasFocusData)
            Add(ReviewInsightKind.AverageSession, $"平均每次有效专注为 {FormatDuration(dashboard.Current.AverageFocusSeconds)}。", ReviewInsightTone.Neutral);

        return insights;

        void Add(ReviewInsightKind kind, string text, ReviewInsightTone tone)
        {
            if (insights.Count < maximum) insights.Add(new ReviewInsight(kind, text, tone));
        }
    }

    private static T? UniqueMaximum<T>(IEnumerable<T> source) where T : class
    {
        var values = source.Select(item => (Item: item, Value: GetValue(item))).Where(item => item.Value > 0).OrderByDescending(item => item.Value).ToArray();
        return values.Length == 0 || values.Length > 1 && values[0].Value == values[1].Value ? null : values[0].Item;
    }

    private static long GetValue<T>(T item) => item switch
    {
        QuadrantValue value => value.Value,
        DateBucketPoint point => point.Value,
        _ => throw new ArgumentException("Unsupported review metric.", nameof(item))
    };

    private static string FormatDuration(long seconds)
    {
        var duration = TimeSpan.FromSeconds(seconds);
        if (duration.TotalHours >= 1) return $"{(int)duration.TotalHours} 小时 {duration.Minutes} 分";
        return $"{duration.Minutes} 分";
    }
}
