using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>Replace a Product's fields by id within the caller's org.
/// The change goes through <see cref="Product.Update"/>, which returns through the entity's invariants; a
/// refused change is never saved, and the row version makes a concurrent write fail loudly instead of being
/// lost.
/// An id from another org is a not-found, never a hint that the row exists.</summary>
[Slice]
public static class UpdateProduct
{
    public record Input(Guid Id, string Name);

    /// <summary>The request body. The id travels in the route, so a body can never name another row.</summary>
    public record Changes(string Name);

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
        app.MapPut("/product/{id:guid}",
                async (Guid id, Changes changes, AppDb db, CancellationToken ct) =>
                    (await Handle(new Input(id, changes.Name), db, ct)).ToHttp())
            .WithName(nameof(UpdateProduct));
}
