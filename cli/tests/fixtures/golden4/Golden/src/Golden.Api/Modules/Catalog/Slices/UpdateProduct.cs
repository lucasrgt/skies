using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>Update a Product by id, scoped to the caller's tenant. A partial update: only the
/// non-null inputs overwrite their column. An unknown id is a not-found.</summary>
[Slice]
public static class UpdateProduct
{
    public record Input(Guid Id, Guid? TenantId = null);

    public record Output(Guid Id);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, TimeProvider clock, CancellationToken ct)
    {
        var item = await db.Products.FirstOrDefaultAsync(e => e.Id == input.Id, ct);
        if (item is null)
            return Error.NotFound(CatalogErrorCodes.ProductNotFound, "product not found");

        if (input.TenantId is not null)
            item.TenantId = input.TenantId.Value;
        await db.SaveChangesAsync(ct);
        return new Output(item.Id);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPut("/product", async (Input input, AppDb db, TimeProvider clock, CancellationToken ct) =>
            (await Handle(input, db, clock, ct)).ToHttp())
            .RequireAuthorization();
}
