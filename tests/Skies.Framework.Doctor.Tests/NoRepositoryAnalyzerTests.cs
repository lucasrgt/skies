namespace Skies.Framework.Doctor.Tests;

public class NoRepositoryAnalyzerTests
{
    // The EF Core surface the rule recognizes, stubbed so the sources compile without the package.
    private const string Ef = """
        namespace Microsoft.EntityFrameworkCore
        {
            public class DbContext { }
            public class DbSet<T> { }
        }
        class AppDb : Microsoft.EntityFrameworkCore.DbContext { }
        class Order { }
        """;

    [Fact]
    public Task A_slice_reading_the_dbcontext_directly_reports_nothing() =>
        Harness<NoRepositoryAnalyzer>.Verify(Ef + """
            class Deposit
            {
                static void Handle(AppDb db) { _ = db; }
            }
            """);

    // A type whose name merely contains the word elsewhere is fine — only the layer-naming suffix matches.
    [Fact]
    public Task A_type_that_only_contains_the_word_reports_nothing() =>
        Harness<NoRepositoryAnalyzer>.Verify("""
            class RepositoryMetadata { }
            """);

    // The audit's false positive: a vendor client named after the vendor's own noun wraps no context.
    [Fact]
    public Task A_vendor_client_named_repository_reports_nothing() =>
        Harness<NoRepositoryAnalyzer>.Verify("""
            interface IGitHubRepository { string Name { get; } }
            class GitHubRepository : IGitHubRepository
            {
                private readonly System.Net.Http.HttpClient http;
                public GitHubRepository(System.Net.Http.HttpClient http) => this.http = http;
                public string Name => http.ToString()!;
            }
            record PackageRepository(string Url);
            """);

    [Fact]
    public Task A_repository_holding_the_dbcontext_and_its_interface_are_flagged() =>
        Harness<NoRepositoryAnalyzer>.Verify(Ef + """
            interface {|SKY0006:IOrderRepository|} { }
            class {|SKY0006:OrderRepository|} : IOrderRepository
            {
                private readonly AppDb db;
                public OrderRepository(AppDb db) => this.db = db;
            }
            """);

    [Fact]
    public Task A_primary_constructor_or_a_dbset_counts_as_wrapping() =>
        Harness<NoRepositoryAnalyzer>.Verify(Ef + """
            class {|SKY0006:UserRepository|}(AppDb db) { public object Db => db; }
            class {|SKY0006:OrderRepository|} { public Microsoft.EntityFrameworkCore.DbSet<Order> Orders { get; set; } = null!; }
            """);

    [Fact]
    public Task A_unit_of_work_over_the_context_is_flagged() =>
        Harness<NoRepositoryAnalyzer>.Verify(Ef + """
            interface {|SKY0006:IUnitOfWork|} { void Commit(); }
            sealed class {|SKY0006:EfUnitOfWork|}(AppDb db) : IUnitOfWork { public void Commit() => _ = db; }
            """);

    // An unimplemented interface names the layer but wraps nothing yet: it is not the anti-pattern on its own.
    [Fact]
    public Task A_layer_named_interface_without_a_wrapper_reports_nothing() =>
        Harness<NoRepositoryAnalyzer>.Verify("""
            interface IUserRepository { }
            """);
}
