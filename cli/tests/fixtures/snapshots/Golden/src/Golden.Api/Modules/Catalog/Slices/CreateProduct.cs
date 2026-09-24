namespace Golden.Api.Modules.Catalog;

/// <summary>Create a Product for the caller's tenant. The slice never writes a column: the entity is born
/// through <see cref="Product.Open"/>, whose invariants (EnsureValid) decide whether it may exist, and the org
/// is stamped by the DbContext, never taken from the request.</summary>
[Slice]
public static class CreateProduct
{
    public record Input(string Name);

    public record Output(Guid Id);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var opened = Product.Open(Guid.NewGuid(), input.Name);
        if (opened.IsFailure)
            return opened.Error;

        db.Products.Add(opened.Value);
        await db.SaveChangesAsync(ct);
        return new Output(opened.Value.Id);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/product", async (Input input, AppDb db, CancellationToken ct) =>
            (await Handle(input, db, ct)).ToHttp())
            .WithName(nameof(CreateProduct))
            .RequireAuthorization();
}
