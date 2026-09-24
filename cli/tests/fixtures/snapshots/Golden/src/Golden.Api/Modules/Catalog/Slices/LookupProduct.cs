using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Catalog;

/// <summary>Look up one Product by id within the caller's org.
/// An id from another org is a not-found, never a hint that the row exists.</summary>
[Slice]
public static class LookupProduct
{
    public record Input(Guid Id);

    public record Output(ProductView Product);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var item = await db.Products.FirstOrDefaultAsync(e => e.Id == input.Id, ct);
        if (item is null)
            return Error.NotFound(CatalogErrorCodes.ProductNotFound, "product not found");

        return new Output(ProductView.From(item));
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapGet("/products/{id:guid}", async (Guid id, AppDb db, CancellationToken ct) =>
                (await Handle(new Input(id), db, ct)).ToHttp())
            .WithName(nameof(LookupProduct));
}
