using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>Replace a Product's fields by id, scoped to the caller's tenant. The change goes through
/// <see cref="Product.Update"/>, which returns through the entity's invariants (EnsureValid); a refused change
/// is never saved. The entity's RowVersion makes a concurrent write fail loudly instead of being lost. An unknown
/// id is a not-found, never a hint that the row exists in another org.</summary>
[Slice]
public static class UpdateProduct
{
    public record Input(Guid Id, string Name);

    public record Output(Guid Id);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var item = await db.Products.FirstOrDefaultAsync(e => e.Id == input.Id, ct);
        if (item is null)
            return Error.NotFound(CatalogErrorCodes.ProductNotFound, "product not found");

        var updated = item.Update(input.Name);
        if (updated.IsFailure)
            return updated.Error;

        await db.SaveChangesAsync(ct);
        return new Output(item.Id);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPut("/product", async (Input input, AppDb db, CancellationToken ct) =>
            (await Handle(input, db, ct)).ToHttp())
            .WithName(nameof(UpdateProduct))
            .RequireAuthorization();
}
