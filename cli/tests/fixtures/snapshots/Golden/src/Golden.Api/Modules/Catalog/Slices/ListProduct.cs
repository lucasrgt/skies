using Skies.Framework.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>List Product records for the caller's tenant, one page at a time. The tenant filter on the
/// DbContext scopes the rows, and the count and the page run over the same query, so neither crosses an org
/// boundary. The order ends on the unique Id, so page boundaries are deterministic.</summary>
[Slice]
public static class ListProduct
{
    public record Input(int Page = 1, int PageSize = 20);

    public record Output(Page<Product> Items);

    private const int MaxPageSize = 100;

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var items = await db.Products.OrderBy(e => e.Id)
            .ToPageAsync(input.Page, input.PageSize, MaxPageSize, ct);
        return new Output(items);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapGet("/product", async (int? page, int? pageSize, AppDb db, CancellationToken ct) =>
            (await Handle(new Input(page ?? 1, pageSize ?? 20), db, ct)).ToHttp())
            .WithName(nameof(ListProduct))
            .RequireAuthorization();
}
