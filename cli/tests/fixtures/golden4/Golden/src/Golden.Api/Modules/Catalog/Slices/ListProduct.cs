using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>List Product records for the caller's tenant, newest first. Authenticated; the tenant
/// filter on the DbContext scopes the rows, so paging never crosses an org boundary.</summary>
[Slice]
public static class ListProduct
{
    public record Input(int? Limit, int? Offset);

    public record Output(IReadOnlyList<Product> Items);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var offset = input.Offset is > 0 ? input.Offset.Value : 0;
        var limit = input.Limit is > 0 ? input.Limit.Value : 50;
        var items = await db.Products
            .OrderByDescending(e => e.Id)
            .Skip(offset)
            .Take(limit)
            .ToListAsync(ct);
        return new Output(items);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapGet("/product", async (int? limit, int? offset, AppDb db, CancellationToken ct) =>
            (await Handle(new Input(limit, offset), db, ct)).ToHttp())
            .RequireAuthorization();
}
