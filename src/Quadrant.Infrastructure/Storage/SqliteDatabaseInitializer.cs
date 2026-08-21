using Microsoft.Data.Sqlite;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteDatabaseInitializer
{
    public const int CurrentSchemaVersion = 3;

    private readonly SqliteConnectionFactory connectionFactory;

    public SqliteDatabaseInitializer(SqliteConnectionFactory connectionFactory)
    {
        this.connectionFactory = connectionFactory ?? throw new ArgumentNullException(nameof(connectionFactory));
    }

    public async Task InitializeAsync(CancellationToken cancellationToken = default)
    {
        var directory = Path.GetDirectoryName(connectionFactory.DatabasePath);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }

        await using var connection = connectionFactory.CreateConnection();
        await connection.OpenAsync(cancellationToken);
        await ConfigureConnectionAsync(connection, cancellationToken);

        await using var transaction = connection.BeginTransaction();
        await ExecuteAsync(connection, transaction, "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);", cancellationToken);

        var version = await ReadSchemaVersionAsync(connection, transaction, cancellationToken);
        if (version > CurrentSchemaVersion)
        {
            throw new InvalidOperationException($"Database schema version {version} is newer than supported version {CurrentSchemaVersion}.");
        }

        if (version == 0)
        {
            await ApplyMigration001Async(connection, transaction, cancellationToken);
            await ExecuteAsync(
                connection,
                transaction,
                "INSERT INTO schema_version (version) VALUES ($version);",
                cancellationToken,
                ("$version", 1));
            version = 1;
        }

        if (version == 1)
        {
            await ApplyMigration002Async(connection, transaction, cancellationToken);
            await ExecuteAsync(connection, transaction, "UPDATE schema_version SET version = $version;", cancellationToken, ("$version", 2));
            version = 2;
        }

        if (version == 2)
        {
            await ApplyMigration003Async(connection, transaction, cancellationToken);
            await EnsureForeignKeysValidAsync(connection, transaction, cancellationToken);
            await ExecuteAsync(connection, transaction, "UPDATE schema_version SET version = $version;", cancellationToken, ("$version", 3));
            version = 3;
        }

        await transaction.CommitAsync(cancellationToken);
    }

    public static async Task ConfigureConnectionAsync(SqliteConnection connection, CancellationToken cancellationToken)
    {
        await ExecuteAsync(connection, null, "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;", cancellationToken);
    }

    private static async Task<int> ReadSchemaVersionAsync(
        SqliteConnection connection,
        SqliteTransaction transaction,
        CancellationToken cancellationToken)
    {
        await using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1;";
        var result = await command.ExecuteScalarAsync(cancellationToken);
        return result is null or DBNull ? 0 : Convert.ToInt32(result, System.Globalization.CultureInfo.InvariantCulture);
    }

    private static Task ApplyMigration001Async(
        SqliteConnection connection,
        SqliteTransaction transaction,
        CancellationToken cancellationToken) =>
        ExecuteAsync(
            connection,
            transaction,
            """
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

            INSERT INTO quadrants (id, name, subtitle) VALUES (1, '重要且紧急', '立即处理');
            INSERT INTO quadrants (id, name, subtitle) VALUES (2, '重要不紧急', '规划推进');
            INSERT INTO quadrants (id, name, subtitle) VALUES (3, '紧急不重要', '简化或委派');
            INSERT INTO quadrants (id, name, subtitle) VALUES (4, '不重要不紧急', '删除或延后');
            """,
            cancellationToken);

    private static Task ApplyMigration002Async(SqliteConnection connection, SqliteTransaction transaction, CancellationToken cancellationToken) =>
        ExecuteAsync(connection, transaction, """
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO settings (key, value) VALUES ('theme', 'System');
            INSERT INTO settings (key, value) VALUES ('close_to_tray', 'true');
            INSERT INTO settings (key, value) VALUES ('launch_at_startup', 'false');
            INSERT INTO settings (key, value) VALUES ('start_minimized', 'false');
            INSERT INTO settings (key, value) VALUES ('global_hotkey', 'Ctrl+Alt+Q');
            """, cancellationToken);

    private static Task ApplyMigration003Async(SqliteConnection connection, SqliteTransaction transaction, CancellationToken cancellationToken) =>
        ExecuteAsync(connection, transaction, """
            CREATE TABLE tasks_v3 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                quadrant_id INTEGER NULL CHECK (quadrant_id IS NULL OR quadrant_id BETWEEN 1 AND 4),
                due_at TEXT NULL,
                reminder_at TEXT NULL,
                note TEXT NULL,
                is_completed INTEGER NOT NULL DEFAULT 0,
                completed_at TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                planned_date TEXT NULL,
                estimated_minutes INTEGER NULL CHECK (estimated_minutes BETWEEN 1 AND 1440),
                recurrence_kind INTEGER NOT NULL DEFAULT 0 CHECK (recurrence_kind BETWEEN 0 AND 3),
                recurrence_interval INTEGER NOT NULL DEFAULT 1 CHECK (recurrence_interval >= 1),
                recurrence_series_id TEXT NULL,
                recurrence_anchor_day INTEGER NULL CHECK (recurrence_anchor_day BETWEEN 1 AND 31),
                FOREIGN KEY (quadrant_id) REFERENCES quadrants(id)
            );

            INSERT INTO tasks_v3 (
                id, title, quadrant_id, due_at, reminder_at, note, is_completed, completed_at, created_at, updated_at,
                planned_date, estimated_minutes, recurrence_kind, recurrence_interval, recurrence_series_id, recurrence_anchor_day)
            SELECT id, title, quadrant_id, due_at, reminder_at, note, is_completed, completed_at, created_at, updated_at,
                NULL, NULL, 0, 1, NULL, NULL
            FROM tasks;

            DROP TABLE tasks;
            ALTER TABLE tasks_v3 RENAME TO tasks;
            CREATE INDEX ix_tasks_active_quadrant ON tasks(is_completed, quadrant_id);
            CREATE INDEX ix_tasks_active_planned ON tasks(is_completed, planned_date);
            CREATE INDEX ix_tasks_due ON tasks(is_completed, due_at);
            CREATE INDEX ix_tasks_recurrence_series ON tasks(recurrence_series_id);

            CREATE TABLE task_completion_events (
                id TEXT PRIMARY KEY,
                task_id INTEGER NULL REFERENCES tasks(id) ON DELETE SET NULL,
                completed_at_utc TEXT NOT NULL,
                completed_local_date TEXT NOT NULL,
                quadrant_snapshot INTEGER NULL CHECK (quadrant_snapshot IS NULL OR quadrant_snapshot BETWEEN 1 AND 4),
                task_title_snapshot TEXT NOT NULL,
                due_at_utc_snapshot TEXT NULL,
                planned_date_snapshot TEXT NULL,
                estimated_minutes_snapshot INTEGER NULL CHECK (estimated_minutes_snapshot BETWEEN 1 AND 1440),
                was_overdue INTEGER NOT NULL DEFAULT 0,
                reverted_at_utc TEXT NULL
            );
            CREATE UNIQUE INDEX ux_completion_active_task ON task_completion_events(task_id) WHERE task_id IS NOT NULL AND reverted_at_utc IS NULL;
            CREATE INDEX ix_completion_local_date_active ON task_completion_events(completed_local_date, reverted_at_utc);
            CREATE INDEX ix_completion_quadrant ON task_completion_events(quadrant_snapshot);

            CREATE TABLE focus_sessions (
                id TEXT PRIMARY KEY,
                task_id INTEGER NULL REFERENCES tasks(id) ON DELETE SET NULL,
                mode INTEGER NOT NULL CHECK (mode BETWEEN 1 AND 2),
                started_at_utc TEXT NOT NULL,
                active_segment_started_utc TEXT NULL,
                ended_at_utc TEXT NULL,
                target_end_at_utc TEXT NULL,
                duration_seconds INTEGER NOT NULL DEFAULT 0 CHECK (duration_seconds >= 0),
                status INTEGER NOT NULL CHECK (status BETWEEN 1 AND 5),
                pomodoro_kind INTEGER NULL CHECK (pomodoro_kind BETWEEN 1 AND 3),
                created_local_date TEXT NOT NULL,
                task_title_snapshot TEXT NULL,
                quadrant_snapshot INTEGER NULL CHECK (quadrant_snapshot IS NULL OR quadrant_snapshot BETWEEN 1 AND 4)
            );
            CREATE INDEX ix_focus_created_local_date ON focus_sessions(created_local_date);
            CREATE INDEX ix_focus_task ON focus_sessions(task_id);
            CREATE INDEX ix_focus_status_started ON focus_sessions(status, started_at_utc);
            """, cancellationToken);

    private static async Task EnsureForeignKeysValidAsync(SqliteConnection connection, SqliteTransaction transaction, CancellationToken cancellationToken)
    {
        await using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = "PRAGMA foreign_key_check;";
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        if (await reader.ReadAsync(cancellationToken))
        {
            throw new InvalidOperationException("Foreign-key validation failed after schema migration.");
        }
    }

    internal static async Task ExecuteAsync(
        SqliteConnection connection,
        SqliteTransaction? transaction,
        string commandText,
        CancellationToken cancellationToken,
        params (string Name, object Value)[] parameters)
    {
        await using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = commandText;
        foreach (var (name, value) in parameters)
        {
            command.Parameters.AddWithValue(name, value);
        }

        await command.ExecuteNonQueryAsync(cancellationToken);
    }
}
