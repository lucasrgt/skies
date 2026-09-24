using Skies.Framework.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>List Products within the caller's org, one page at a time.
/// The order ends on the unique Id, so page boundaries are deterministic, and the page is projected to
/// <see cref="ProductView"/> after paging, so the entity itself never reaches the wire.</summary>
[Slice]
public static class ListProducts
{
    public record Input(int Page = 1, int PageSize = 20);

    public record Output(Page<ProductView> Products);

    private const int MaxPageSize = 100;

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var page = await db.Products.OrderBy(e => e.Id)
            .ToPageAsync(input.Page, input.PageSize, MaxPageSize, ct);
        return new Output(page.Select(ProductView.From));
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapGet("/products", async (int? page, int? pageSize, AppDb db, CancellationToken ct) =>
                (await Handle(new Input(page ?? 1, pageSize ?? 20), db, ct)).ToHttp())
            .WithName(nameof(ListProducts));
}
