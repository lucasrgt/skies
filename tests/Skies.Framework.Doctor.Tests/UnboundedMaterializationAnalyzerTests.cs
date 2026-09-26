namespace Skies.Framework.Doctor.Tests;

public class UnboundedMaterializationAnalyzerTests
{
    [Fact]
    public Task A_dbset_materialized_with_no_bound_is_flagged() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class ListAll
            {
                static async System.Threading.Tasks.Task Handle(Db db, System.Threading.CancellationToken ct)
                {
                    var all = await db.Wallets.{|SKY0027:ToListAsync|}(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_take_in_the_chain_is_the_bound() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class ListSome
            {
                private const int PageSize = 20;

                static async System.Threading.Tasks.Task Handle(Db db, System.Threading.CancellationToken ct)
                {
                    var some = await db.Wallets.OrderBy(w => w.Id).Skip(20).Take(PageSize).ToListAsync(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_bare_number_as_the_bound_asks_for_a_named_one() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class ListGuess
            {
                static async System.Threading.Tasks.Task Handle(Db db, System.Threading.CancellationToken ct)
                {
                    var some = await db.Wallets.OrderBy(w => w.Id).Take({|SKY0027:500|}).ToListAsync(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_dbset_rooted_local_with_no_bound_anywhere_is_flagged() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class Search
            {
                static async System.Threading.Tasks.Task Handle(Db db, string? q, System.Threading.CancellationToken ct)
                {
                    var query = db.Wallets.AsQueryable();
                    if (q is not null)
                        query = query.Where(w => w.Id.ToString() == q);
                    var hits = await query.{|SKY0027:ToListAsync|}(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_bound_on_any_assignment_to_the_local_counts() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class Search
            {
                static async System.Threading.Tasks.Task Handle(Db db, System.Threading.CancellationToken ct)
                {
                    var query = db.Wallets.AsQueryable();
                    query = query.Take(50);
                    var hits = await query.ToListAsync(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task An_in_memory_queryable_local_is_not_a_database_query() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class Tally
            {
                static async System.Threading.Tasks.Task Handle(System.Collections.Generic.List<int> ids, System.Threading.CancellationToken ct)
                {
                    var query = ids.AsQueryable();
                    var all = await query.ToListAsync(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_parent_scoped_where_is_the_bound() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class StepsOfOneJob
            {
                static async System.Threading.Tasks.Task Handle(Db db, System.Guid jobId, System.Threading.CancellationToken ct)
                {
                    var steps = await db.Wallets.Where(w => w.OwnerId == jobId && w.Id != jobId).ToListAsync(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_contains_match_on_a_parent_key_is_the_bound() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using System.Collections.Generic;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class NamesForThePage
            {
                static async System.Threading.Tasks.Task Handle(Db db, List<System.Guid> ids, System.Threading.CancellationToken ct)
                {
                    var rows = await db.Wallets.Where(w => ids.Contains(w.OwnerId)).ToListAsync(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task Tenant_scope_equality_is_not_a_parent_bound() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class WholeTenant
            {
                static async System.Threading.Tasks.Task Handle(Db db, System.Guid org, System.Threading.CancellationToken ct)
                {
                    var all = await db.Wallets.Where(w => w.OrgId == org).{|SKY0027:ToListAsync|}(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_non_key_filter_is_not_a_bound() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class LiveRows
            {
                static async System.Threading.Tasks.Task Handle(Db db, System.Threading.CancellationToken ct)
                {
                    var all = await db.Wallets.Where(w => w.DeletedAt == null).{|SKY0027:ToListAsync|}(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_parent_key_on_the_value_side_grants_nothing() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class TenantRoster
            {
                public record Input(System.Guid AgencyId);

                static async System.Threading.Tasks.Task Handle(Db db, Input input, System.Threading.CancellationToken ct)
                {
                    // The ENTITY side is the tenant scope (OrgId); input.AgencyId ending in Id must not exempt it.
                    var all = await db.Wallets.Where(w => w.OrgId == input.AgencyId).{|SKY0027:ToListAsync|}(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task A_group_by_aggregation_is_its_own_bound() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using System.Linq;
            using Skies.Framework.Abstractions;
            using Microsoft.EntityFrameworkCore;

            [Slice]
            static class Rollup
            {
                static async System.Threading.Tasks.Task Handle(Db db, System.Threading.CancellationToken ct)
                {
                    var counts = await db.Wallets.GroupBy(w => w.OrgId).Select(g => g.Count()).ToListAsync(ct);
                }
            }
            """ + Stubs);

    [Fact]
    public Task Outside_a_slice_the_rule_stays_silent() =>
        Harness<UnboundedMaterializationAnalyzer>.Verify("""
            using Microsoft.EntityFrameworkCore;

            static class Seeder
            {
                static async System.Threading.Tasks.Task Run(Db db, System.Threading.CancellationToken ct)
                {
                    var all = await db.Wallets.ToListAsync(ct);
                }
            }
            """ + Stubs);

    // A tiny stand-in for the EF + Skies surface so the tests need no package references; the rule
    // matches DbSet/IQueryable by type name and the chain shape syntactically.
    private const string Stubs = """


        namespace Skies.Framework.Abstractions
        {
            public sealed class SliceAttribute : System.Attribute { }
        }

        namespace Microsoft.EntityFrameworkCore
        {
            public class DbSet<T> : System.Linq.IQueryable<T>
            {
                public System.Type ElementType => typeof(T);
                public System.Linq.Expressions.Expression Expression => null!;
                public System.Linq.IQueryProvider Provider => null!;
                public System.Collections.Generic.IEnumerator<T> GetEnumerator() => null!;
                System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator() => null!;
            }

            public static class EfQueryableExtensions
            {
                public static System.Threading.Tasks.Task<System.Collections.Generic.List<T>> ToListAsync<T>(
                    this System.Linq.IQueryable<T> query, System.Threading.CancellationToken ct = default) => null!;
            }
        }

        class Db
        {
            public Microsoft.EntityFrameworkCore.DbSet<Wallet> Wallets => null!;
        }

        class Wallet
        {
            public System.Guid Id { get; set; }
            public System.Guid OwnerId { get; set; }
            public System.Guid OrgId { get; set; }
            public System.DateTime? DeletedAt { get; set; }
        }
        """;
}
