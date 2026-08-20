# Stage 08 — All / Today / Overdue, Search, Completed History

## Goal

防止四象限任务增长后变成四个垃圾桶，同时保持认知模型不变。

## Implementation

### Filter state

MainViewModel 只有一个 `SelectedFilter`：All / Today / Overdue。

过滤后仍然是四个 QuadrantVM。不要创建 TodayPage / OverduePage。

### Time rules

通过 `IClock`：

- Today：DueAt 转系统本地日期 == current local date；
- Overdue：DueAt < now && !completed。

### Search

顶部 TextBox：Title + Note case-insensitive contains。

`Ctrl+F`：focus search。

小数据量直接内存过滤；不加 debounce timer。1000 tasks 若 Stage 16 测到问题再改。

组合规则：filter 与 search 同时生效。

### Completed window

单独简洁 window/list：

- load completed order by CompletedAt desc；
- restore；
- permanent delete；
- 搜索可不做，除非 Stage 很轻且 SPEC 不冲突；默认不做。

恢复后回到原 `QuadrantId`。

### Due visual

只在 Date text / small status 上表达 Today/Overdue；不把整个 card 染红。

## Tests

Core filter tests：跨午夜边界、completed excluded、null Due。

Manual：

- filter + search combo；
- completed restore 后重新出现正确 quadrant；
- overdue accent 不只靠颜色，至少有文字/图标语义；
- `Ctrl+F`, Esc 清理行为一致。

## DO NOT

- 不做统计；
- 不做 calendar；
- 不做 advanced query language；
- 不把 completed 永久加载到 active collections。

## Handoff

STATUS 记录过滤/搜索在内存完成。下一 Stage 09。
