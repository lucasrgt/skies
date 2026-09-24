using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>Look up one Product by id, scoped to the caller's tenant. An unknown id is a not-found,
/// never a hint that the row exists in another org.</summary>
[Slice]
public static class LookupProduct
{
    public record Input(Guid Id);

    public record Output(Product Item);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var item = await db.Products.FirstOrDefaultAsync(e => e.Id == input.Id, ct);
        return item is null ? Error.NotFound(CatalogErrorCodes.ProductNotFound, "product not found") : new Output(item);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapGet("/product/{id:guid}", async (Guid id, AppDb db, CancellationToken ct) =>
            (await Handle(new Input(id), db, ct)).ToHttp())
            .WithName(nameof(LookupProduct))
            .RequireAuthorization();
}
