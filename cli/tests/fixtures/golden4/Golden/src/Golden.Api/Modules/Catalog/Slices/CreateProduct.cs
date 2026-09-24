namespace Golden.Api.Modules.Catalog;

/// <summary>Create a Product for the caller's tenant. The org is stamped by the DbContext; only the
/// writable scalar fields are taken from the request.</summary>
[Slice]
public static class CreateProduct
{
    public record Input(Guid TenantId);

    public record Output(Guid Id);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, TimeProvider clock, CancellationToken ct)
    {
        var item = new Product
        {
            Id = Guid.NewGuid(),
            TenantId = input.TenantId,
        };
        db.Products.Add(item);
        await db.SaveChangesAsync(ct);
        return new Output(item.Id);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/product", async (Input input, AppDb db, TimeProvider clock, CancellationToken ct) =>
            (await Handle(input, db, clock, ct)).ToHttp())
            .RequireAuthorization();
}
