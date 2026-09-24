namespace Golden.Api.Modules.Billing;

/// <summary>GetInvoice — fill in the operation; the "why" lives here until it outgrows a header.</summary>
[Slice]
public static class GetInvoice
{
    public record Input(Guid Id);
    public record Output(Guid Id);

    public static Task<Result<Output>> Handle(Input input, CancellationToken ct)
    {
        var validation = new Validation()
            .Require(input.Id, "id", BillingErrorCodes.IdRequired);
        if (validation.Failed)
            return Task.FromResult<Result<Output>>(validation.ToError());

        return Task.FromResult<Result<Output>>(new Output(input.Id));
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/getinvoice", async (Input input, CancellationToken ct) =>
                (await Handle(input, ct)).ToHttp())
            .WithName(nameof(GetInvoice)); // authorization: the module's route group decides (SKY0022)
}
