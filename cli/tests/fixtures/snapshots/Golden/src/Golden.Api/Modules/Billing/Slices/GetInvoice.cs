namespace Golden.Api.Modules.Billing;

[Slice]
public static class GetInvoice
{
    public record Input(Guid Id);

    public record Output();

    public static Task<Result<Output>> Handle(Input input, CancellationToken ct) =>
        Task.FromResult<Result<Output>>(
            Error.BusinessRule(BillingErrorCodes.GetInvoiceNotImplemented, "GetInvoice is not implemented yet."));

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/getinvoice", async (Input input, CancellationToken ct) =>
                (await Handle(input, ct)).ToHttp())
            .WithName(nameof(GetInvoice)); // authorization: the module's route group decides
}
