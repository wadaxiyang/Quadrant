# Stage 04 — SQLite Storage, Schema Versioning, Repository Tests

## Goal

实现稳定本地持久化。此 Stage 结束后，数据库可以完整 CRUD 任务和四象限配置，但 UI 尚不负责编辑。

## Before coding — MUST browse

重新查：

- `Microsoft.Data.Sqlite` 当前 stable 与 ADO.NET 用法；
- 官方 transaction 文档；
- connection string / parameter API；
- SQLite foreign_keys 行为。

## Technical implementation

### AppData path

`LocalAppDataPathProvider`：

```text
%LOCALAPPDATA%\Quadrant\quadrant.db
```

使用 `Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData)`，不要硬编码 `C:\Users`。

### Db initializer

`SqliteDatabaseInitializer`：

- create directory；
- open connection；
- `PRAGMA foreign_keys = ON` per connection；
- 可设置合理 `busy_timeout`；
- 创建/读取 `schema_version`；
- 顺序执行 migration。

Migration 001 创建 `quadrants`, `tasks`, indexes，schema 与 ARCHITECTURE 一致。

默认象限用 migration insert，ID 永远 1–4。

### Repository

`SqliteTaskRepository`：所有 SQL 参数化。

不要 ORM mapping library。写少量 private mapping helper：`SqliteDataReader -> TaskItem`。

时间统一：

```csharp
value.ToString("O", CultureInfo.InvariantCulture)
DateTimeOffset.Parse(..., CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind)
```

具体 API 如需调整可按 .NET 10 官方行为。

### Transaction

多语句 mutation / migration 必须 transaction。单条简单 UPDATE 可直接执行，但 TaskService later 负责跨系统 side effect。

## Tests

Infrastructure tests 使用**每个 test 独立临时数据库文件**，不要共享用户 `%LOCALAPPDATA%`。

覆盖：

- fresh database migration；
- default quadrants；
- create/read/update/delete；
- nullable due/reminder/note；
- DateTimeOffset round-trip；
- complete/restore；
- migration repeated startup idempotent；
- SQL title 中含 `'` 不出错（验证 parameterization）。

测试结束清理 temp dir。

## DO NOT

- 不用 EF Core；
- 不启用复杂 WAL tuning；
- 不做 cache layer；
- 不做 backup/sync；
- 不碰 notification。

## Acceptance

`dotnet test` 全绿；人为删除 DB 后 app 下次可重建；Repository 无字符串插值 SQL 值。

## Handoff

STATUS 写 schema version=1、DB 路径、测试结果。下一阶段 Stage 05。
