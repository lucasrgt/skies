using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>Delete a Product by id, scoped to the caller's tenant (hard delete). An unknown id is a
/// not-found, never a hint that the row exists in another org.</summary>
[Slice]
public static class DeleteProduct
{
    public record Input(Guid Id);

    public record Output();

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var item = await db.Products.FirstOrDefaultAsync(e => e.Id == input.Id, ct);
        if (item is null)
            return Error.NotFound(CatalogErrorCodes.ProductNotFound, "product not found");

        db.Products.Remove(item);
        await db.SaveChangesAsync(ct);
        return new Output();
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapDelete("/product/{id:guid}", async (Guid id, AppDb db, CancellationToken ct) =>
            (await Handle(new Input(id), db, ct)).ToHttp())
            .RequireAuthorization();
}
