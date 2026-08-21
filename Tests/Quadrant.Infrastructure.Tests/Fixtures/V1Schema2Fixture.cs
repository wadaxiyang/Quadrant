using Microsoft.Data.Sqlite;

namespace Quadrant.Infrastructure.Tests.Fixtures;

internal static class V1Schema2Fixture
{
    public static async Task CreateAsync(string databasePath, CancellationToken cancellationToken = default)
    {
        var connectionString = new SqliteConnectionStringBuilder
        {
            DataSource = databasePath,
            Mode = SqliteOpenMode.ReadWriteCreate,
            Pooling = false
        }.ToString();

        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            PRAGMA foreign_keys = ON;
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (2);

            CREATE TABLE quadrants (
                id INTEGER PRIMARY KEY CHECK (id BETWEEN 1 AND 4),
                name TEXT NOT NULL,
                subtitle TEXT NOT NULL
            );

            CREATE TABLE tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                quadrant_id INTEGER NOT NULL,
                due_at TEXT NULL,
                reminder_at TEXT NULL,
                note TEXT NULL,
                is_completed INTEGER NOT NULL DEFAULT 0,
                completed_at TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (quadrant_id) REFERENCES quadrants(id)
            );

            CREATE INDEX ix_tasks_active_quadrant ON tasks(is_completed, quadrant_id);
            CREATE INDEX ix_tasks_due ON tasks(is_completed, due_at);
            CREATE INDEX ix_tasks_reminder ON tasks(is_completed, reminder_at);

            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

            INSERT INTO quadrants (id, name, subtitle) VALUES
                (1, '重要且紧急', '立即处理'),
                (2, '重要不紧急', '规划推进'),
                (3, '紧急不重要', '简化或委派'),
                (4, '不重要不紧急', '删除或延后');

            INSERT INTO settings (key, value) VALUES
                ('theme', 'System'),
                ('close_to_tray', 'true'),
                ('launch_at_startup', 'false'),
                ('start_minimized', 'false'),
                ('global_hotkey', 'Ctrl+Alt+Q');

            INSERT INTO tasks (id, title, quadrant_id, due_at, reminder_at, note, is_completed, completed_at, created_at, updated_at) VALUES
                (101, '中文活动任务', 1, '2026-08-25T10:00:00.0000000+08:00', '2026-08-24T09:00:00.0000000+08:00', '含中文与提醒', 0, NULL, '2026-08-21T08:00:00.0000000+08:00', '2026-08-21T08:00:00.0000000+08:00'),
                (102, 'Completed task', 2, NULL, NULL, 'Completed note', 1, '2026-08-20T16:30:00.0000000+08:00', '2026-08-19T08:00:00.0000000+08:00', '2026-08-20T16:30:00.0000000+08:00'),
                (103, 'Reminder only', 4, NULL, '2026-08-23T11:00:00.0000000+08:00', NULL, 0, NULL, '2026-08-21T08:30:00.0000000+08:00', '2026-08-21T08:30:00.0000000+08:00');
            """;
        await command.ExecuteNonQueryAsync(cancellationToken);
    }
}
