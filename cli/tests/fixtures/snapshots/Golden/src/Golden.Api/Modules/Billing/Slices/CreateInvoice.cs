namespace Golden.Api.Modules.Billing;

[Slice]
public static class CreateInvoice
{
    public record Input(Guid Id);

    public record Output();

    public static Task<Result<Output>> Handle(Input input, CancellationToken ct) =>
        Task.FromResult<Result<Output>>(
            Error.BusinessRule(BillingErrorCodes.CreateInvoiceNotImplemented, "CreateInvoice is not implemented yet."));

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/createinvoice", async (Input input, CancellationToken ct) =>
                (await Handle(input, ct)).ToHttp())
            .WithName(nameof(CreateInvoice)); // authorization: the module's route group decides
}
