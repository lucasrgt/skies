using System.Linq;
using System.Text.Json;
using Sample.Tests;

namespace Specs.S0005;

// The wire's actual types, document-wide. NumberHandling's read-from-string tolerance is a runtime affordance the
// serializer never writes, so it must not leak into the contract as `type: ["number","string"]`: a client generator
// faithfully turns that into `number | string` and every ViewModel grows a Number(x) || 0 coercion. Nullability is
// the one union allowed to survive. The page is matched STRUCTURALLY by the frontend spine's pager hooks.
public class SchemaContractSpec
{
    [Fact(DisplayName = "FM-3: a numeric body property is a plain number")]
    public async Task A_numeric_body_property_is_plainly_typed()
    {
        using var document = await Contract();
        var balance = Schema(document, "ListWalletsWalletView").GetProperty("properties").GetProperty("balance");

        Assert.Equal(JsonValueKind.String, balance.GetProperty("type").ValueKind);
        Assert.Equal("number", balance.GetProperty("type").GetString());
        Assert.False(balance.TryGetProperty("pattern", out _));
    }

    // WalletView.LastDeposit is the document's ONLY Money occurrence and it is Money?, so the Money component is created
    // from the Nullable<Money> visit: the mirror must unwrap Nullable to find the ScalarJsonConverter (a pilot's
    // nullable-only scalars leaked as bare {}).
    [Fact(DisplayName = "FM-4: a scalar value object reached only through a nullable property mirrors its primitive")]
    public async Task A_nullable_only_scalar_value_object_mirrors_its_primitive()
    {
        using var document = await Contract();
        var money = Schema(document, "Money");

        Assert.Equal("number", money.GetProperty("type").GetString());
        Assert.False(money.TryGetProperty("properties", out _));
    }

    [Fact(DisplayName = "FM-5: a numeric query parameter accepts no strings")]
    public async Task A_numeric_query_parameter_is_plainly_typed()
    {
        using var document = await Contract();
        var operation = document.RootElement.GetProperty("paths").EnumerateObject()
            .SelectMany(path => path.Value.EnumerateObject())
            .First(entry => entry.Value.ValueKind == JsonValueKind.Object
                && entry.Value.TryGetProperty("operationId", out var id)
                && id.GetString() == "ListWallets")
            .Value;
        var page = operation.GetProperty("parameters").EnumerateArray()
            .First(parameter => parameter.GetProperty("name").GetString() == "page")
            .GetProperty("schema");

        var types = page.GetProperty("type").ValueKind == JsonValueKind.Array
            ? page.GetProperty("type").EnumerateArray().Select(t => t.GetString()).ToList()
            : [page.GetProperty("type").GetString()];
        Assert.Contains("integer", types);
        Assert.DoesNotContain("string", types);
        Assert.False(page.TryGetProperty("pattern", out _));
    }

    [Fact(DisplayName = "FM-6: the page's four members are required, plain array and integers")]
    public async Task The_page_schema_is_required_and_plainly_typed()
    {
        using var document = await Contract();
        var page = Schema(document, "PageOfListWalletsWalletView");
        var properties = page.GetProperty("properties");

        var required = page.GetProperty("required").EnumerateArray().Select(v => v.GetString()).ToList();
        Assert.Equal(["items", "totalCount", "pageNumber", "pageSize"], required);
        Assert.Equal(JsonValueKind.String, properties.GetProperty("items").GetProperty("type").ValueKind);
        Assert.Equal("array", properties.GetProperty("items").GetProperty("type").GetString());
        Assert.Equal("integer", properties.GetProperty("totalCount").GetProperty("type").GetString());
        Assert.Equal("integer", properties.GetProperty("pageNumber").GetProperty("type").GetString());
        Assert.Equal("integer", properties.GetProperty("pageSize").GetProperty("type").GetString());
    }

    [Fact(DisplayName = "FM-7: a slice's output references its own slice-qualified page")]
    public async Task The_slice_output_references_its_qualified_page()
    {
        using var document = await Contract();
        var wallets = Schema(document, "ListWalletsOutput").GetProperty("properties").GetProperty("wallets");

        Assert.Equal("#/components/schemas/PageOfListWalletsWalletView", wallets.GetProperty("$ref").GetString());
    }

    private static async Task<JsonDocument> Contract()
    {
        await using var app = new TestApp();
        using var client = app.CreateClient();
        return JsonDocument.Parse(await client.GetStringAsync("/openapi/v1.json"));
    }

    private static JsonElement Schema(JsonDocument document, string name) =>
        document.RootElement.GetProperty("components").GetProperty("schemas").GetProperty(name);
}
