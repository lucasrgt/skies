using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>Delete a Product by id within the caller's org, as of the version the client read (<c>?version=</c>).
/// Removing a row is a persistence act, not a state transition, so there is no entity method to call. The delete only
/// succeeds while the row is still at that version: a change made in between answers a conflict, so a row is never
/// removed on the strength of a stale read.
/// An id from another org is a not-found, never a hint that the row exists.</summary>
[Slice]
public static class DeleteProduct
{
    public record Input(Guid Id, Guid Version);

    public record Output();

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var item = await db.Products.FirstOrDefaultAsync(e => e.Id == input.Id, ct);
        if (item is null)
            return Error.NotFound(CatalogErrorCodes.ProductNotFound, "product not found");

        db.Entry(item).Property(e => e.Version).OriginalValue = input.Version;
        db.Products.Remove(item);
        try
        {
            await db.SaveChangesAsync(ct);
        }
        catch (DbUpdateConcurrencyException)
        {
            return Error.Conflict(CatalogErrorCodes.ProductChanged, "product changed since it was read");
        }
        return new Output();
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapDelete("/products/{id:guid}", async (Guid id, Guid version, AppDb db, CancellationToken ct) =>
                (await Handle(new Input(id, version), db, ct)).ToHttp())
            .WithName(nameof(DeleteProduct));
}
