using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>Replace a Product's fields by id within the caller's org, as of the version the client read.
/// The change goes through <see cref="Product.Update"/>, which returns through the entity's invariants, so a
/// refused change is never saved. The save only succeeds while the row is still at <c>Version</c>: a change made in
/// between answers a conflict instead of being silently overwritten.
/// An id from another org is a not-found, never a hint that the row exists.</summary>
[Slice]
public static class UpdateProduct
{
    public record Input(Guid Id, string Name, Guid Version);

    /// <summary>The request body. The id travels in the route, so a body can never name another row.</summary>
    public record Changes(string Name, Guid Version);

    public record Output(Guid Id, Guid Version);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var item = await db.Products.FirstOrDefaultAsync(e => e.Id == input.Id, ct);
        if (item is null)
            return Error.NotFound(CatalogErrorCodes.ProductNotFound, "product not found");

        // The version the client read is what the save compares against the stored row.
        db.Entry(item).Property(e => e.Version).OriginalValue = input.Version;
        var updated = item.Update(input.Name);
        if (updated.IsFailure)
            return updated.Error;

        try
        {
            await db.SaveChangesAsync(ct);
        }
        catch (DbUpdateConcurrencyException)
        {
            return Error.Conflict(CatalogErrorCodes.ProductChanged, "product changed since it was read");
        }
        return new Output(item.Id, item.Version);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPut("/products/{id:guid}",
                async (Guid id, Changes changes, AppDb db, CancellationToken ct) =>
                    (await Handle(new Input(id, changes.Name, changes.Version), db, ct)).ToHttp())
            .WithName(nameof(UpdateProduct));
}
