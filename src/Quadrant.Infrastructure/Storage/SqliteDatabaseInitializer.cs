using Microsoft.Data.Sqlite;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteDatabaseInitializer
{
    public const int CurrentSchemaVersion = 2;

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
                ("$version", CurrentSchemaVersion));
            version = 1;
        }

        if (version == 1)
        {
            await ApplyMigration002Async(connection, transaction, cancellationToken);
            await ExecuteAsync(connection, transaction, "UPDATE schema_version SET version = $version;", cancellationToken, ("$version", 2));
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
