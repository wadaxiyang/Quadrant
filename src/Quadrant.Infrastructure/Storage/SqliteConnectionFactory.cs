using Microsoft.Data.Sqlite;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteConnectionFactory
{
    public string DatabasePath { get; }

    public SqliteConnectionFactory(string databasePath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(databasePath);
        DatabasePath = databasePath;
    }

    public SqliteConnection CreateConnection()
    {
        var builder = new SqliteConnectionStringBuilder
        {
            DataSource = DatabasePath,
            Mode = SqliteOpenMode.ReadWriteCreate,
            Cache = SqliteCacheMode.Default,
            Pooling = false
        };

        return new SqliteConnection(builder.ToString());
    }
}
