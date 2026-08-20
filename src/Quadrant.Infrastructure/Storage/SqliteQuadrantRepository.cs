using Microsoft.Data.Sqlite;
using Quadrant.Core.Interfaces;
using Quadrant.Core.Models;

namespace Quadrant.Infrastructure.Storage;

public sealed class SqliteQuadrantRepository : IQuadrantRepository
{
    private readonly SqliteConnectionFactory connectionFactory;

    public SqliteQuadrantRepository(SqliteConnectionFactory connectionFactory)
    {
        this.connectionFactory = connectionFactory ?? throw new ArgumentNullException(nameof(connectionFactory));
    }

    public async Task<IReadOnlyList<QuadrantDefinition>> GetAllAsync(CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "SELECT id, name, subtitle FROM quadrants ORDER BY id;";
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var quadrants = new List<QuadrantDefinition>();
        while (await reader.ReadAsync(cancellationToken))
        {
            quadrants.Add(new QuadrantDefinition(reader.GetInt32(0), reader.GetString(1), reader.GetString(2)));
        }

        return quadrants;
    }

    public async Task<QuadrantDefinition?> GetByIdAsync(int id, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "SELECT id, name, subtitle FROM quadrants WHERE id = $id;";
        command.Parameters.AddWithValue("$id", id);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        return await reader.ReadAsync(cancellationToken)
            ? new QuadrantDefinition(reader.GetInt32(0), reader.GetString(1), reader.GetString(2))
            : null;
    }

    public async Task UpdateAsync(QuadrantDefinition quadrant, CancellationToken cancellationToken = default)
    {
        await using var connection = await OpenConnectionAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "UPDATE quadrants SET name = $name, subtitle = $subtitle WHERE id = $id;";
        command.Parameters.AddWithValue("$id", quadrant.Id);
        command.Parameters.AddWithValue("$name", quadrant.Name);
        command.Parameters.AddWithValue("$subtitle", quadrant.Subtitle);
        if (await command.ExecuteNonQueryAsync(cancellationToken) == 0)
        {
            throw new InvalidOperationException($"Quadrant {quadrant.Id} was not found.");
        }
    }

    private async Task<SqliteConnection> OpenConnectionAsync(CancellationToken cancellationToken)
    {
        var connection = connectionFactory.CreateConnection();
        try
        {
            await connection.OpenAsync(cancellationToken);
            await SqliteDatabaseInitializer.ConfigureConnectionAsync(connection, cancellationToken);
            return connection;
        }
        catch
        {
            await connection.DisposeAsync();
            throw;
        }
    }
}
