using Microsoft.Data.Sqlite;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteConnectionFactory
{
    private readonly bool pooling;

    public string DatabasePath { get; }

    public SqliteConnectionFactory(string databasePath, bool pooling = true)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(databasePath);
        DatabasePath = databasePath;
        this.pooling = pooling;
    }

    public SqliteConnection CreateConnection()
    {
        var builder = new SqliteConnectionStringBuilder
        {
            DataSource = DatabasePath,
            Mode = SqliteOpenMode.ReadWriteCreate,
            Cache = SqliteCacheMode.Default,
            Pooling = pooling
        };

        return new SqliteConnection(builder.ToString());
    }
}
