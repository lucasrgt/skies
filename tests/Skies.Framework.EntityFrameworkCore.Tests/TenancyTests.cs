using Microsoft.EntityFrameworkCore;

namespace Skies.Framework.EntityFrameworkCore.Tests;

public class TenancyTests
{
    private static readonly Guid OrgA = Guid.NewGuid();
    private static readonly Guid OrgB = Guid.NewGuid();

    [Fact]
    public async Task Reads_see_only_the_current_orgs_rows_with_one_cached_model()
    {
        var store = Guid.NewGuid().ToString();
        await Seed(store, OrgA, "a");
        await Seed(store, OrgB, "b");

        // Two contexts of one type share EF's cached model; each must still read its own org.
        await using var asA = new NotesDb(store, new FixedTenant(OrgA));
        await using var asB = new NotesDb(store, new FixedTenant(OrgB));

        Assert.Equal(["a"], await asA.Notes.Select(n => n.Text).ToListAsync());
        Assert.Equal(["b"], await asB.Notes.Select(n => n.Text).ToListAsync());
        Assert.Equal(2, await asA.Notes.IgnoreQueryFilters([TenantModelBuilderExtensions.FilterName]).CountAsync());
    }

    [Fact]
    public async Task A_unit_of_work_without_an_org_reads_nothing()
    {
        var store = Guid.NewGuid().ToString();
        await Seed(store, OrgA, "a");

        await using var anonymous = new NotesDb(store, new FixedTenant(Guid.Empty));

        Assert.Empty(await anonymous.Notes.ToListAsync());
    }

    [Fact]
    public async Task Async_and_sync_saves_both_stamp_the_current_org()
    {
        var store = Guid.NewGuid().ToString();
        await using var db = new NotesDb(store, new FixedTenant(OrgA));

        var viaAsync = Note.Write("async");
        db.Notes.Add(viaAsync);
        await db.SaveChangesAsync();
        var viaSync = Note.Write("sync");
        db.Notes.Add(viaSync);
        db.SaveChanges();

        Assert.Equal(OrgA, viaAsync.OrgId);
        Assert.Equal(OrgA, viaSync.OrgId);
    }

    [Fact]
    public async Task An_insert_with_no_org_to_stamp_is_refused_before_it_is_stored()
    {
        var store = Guid.NewGuid().ToString();
        await using var anonymous = new NotesDb(store, new FixedTenant(Guid.Empty));

        anonymous.Notes.Add(Note.Write("orphan"));

        Assert.Throws<InvalidOperationException>(() => anonymous.SaveChanges());
        await Assert.ThrowsAsync<InvalidOperationException>(() => anonymous.SaveChangesAsync());
        await using var all = new NotesDb(store, new FixedTenant(OrgA));
        Assert.Equal(0, await all.Notes.IgnoreQueryFilters().CountAsync());
    }

    [Fact]
    public async Task A_row_whose_factory_named_its_org_keeps_it()
    {
        // Registration: the anonymous request resolves no org, but the user row names the org it just opened.
        var store = Guid.NewGuid().ToString();
        await using var anonymous = new NotesDb(store, new FixedTenant(Guid.Empty));

        var note = Note.WriteIn(OrgB, "founder");
        anonymous.Notes.Add(note);
        await anonymous.SaveChangesAsync();

        Assert.Equal(OrgB, note.OrgId);
    }

    [Fact]
    public async Task A_row_cannot_move_to_another_org()
    {
        var store = Guid.NewGuid().ToString();
        await Seed(store, OrgA, "a");
        await using var db = new NotesDb(store, new FixedTenant(OrgA));
        var note = await db.Notes.SingleAsync();

        db.Entry(note).Property(n => n.OrgId).CurrentValue = OrgB;

        await Assert.ThrowsAsync<InvalidOperationException>(() => db.SaveChangesAsync());
    }

    [Fact]
    public async Task A_request_in_one_org_cannot_insert_into_another()
    {
        var store = Guid.NewGuid().ToString();
        await using var asA = new NotesDb(store, new FixedTenant(OrgA));

        asA.Notes.Add(Note.WriteIn(OrgB, "planted"));

        var refused = await Assert.ThrowsAsync<InvalidOperationException>(() => asA.SaveChangesAsync());
        Assert.Contains("FixedTenant.System", refused.Message, StringComparison.Ordinal);
        Assert.Throws<InvalidOperationException>(() => asA.SaveChanges());
        Assert.Equal(0, await CountAll(store));
    }

    [Fact]
    public async Task A_request_may_name_its_own_org_on_insert()
    {
        var store = Guid.NewGuid().ToString();
        await using var asA = new NotesDb(store, new FixedTenant(OrgA));

        asA.Notes.Add(Note.WriteIn(OrgA, "mine"));
        await asA.SaveChangesAsync();

        Assert.Equal(1, await CountAll(store));
    }

    [Fact]
    public async Task A_request_in_one_org_cannot_delete_another_orgs_detached_row()
    {
        var store = Guid.NewGuid().ToString();
        var theirs = await Seed(store, OrgB, "b");
        await using var asA = new NotesDb(store, new FixedTenant(OrgA));

        asA.Notes.Remove(theirs);

        await Assert.ThrowsAsync<InvalidOperationException>(() => asA.SaveChangesAsync());
        Assert.Throws<InvalidOperationException>(() => asA.SaveChanges());
        Assert.Equal(1, await CountAll(store));
    }

    [Fact]
    public async Task A_request_in_one_org_cannot_update_another_orgs_row_however_it_got_it()
    {
        var store = Guid.NewGuid().ToString();
        var theirs = await Seed(store, OrgB, "b");

        await using var attached = new NotesDb(store, new FixedTenant(OrgA));
        theirs.Edit("defaced");
        attached.Notes.Update(theirs);
        await Assert.ThrowsAsync<InvalidOperationException>(() => attached.SaveChangesAsync());

        await using var queried = new NotesDb(store, new FixedTenant(OrgA));
        var acrossTheFilter = await queried.Notes.IgnoreQueryFilters().SingleAsync();
        acrossTheFilter.Edit("defaced");
        await Assert.ThrowsAsync<InvalidOperationException>(() => queried.SaveChangesAsync());

        await using var asB = new NotesDb(store, new FixedTenant(OrgB));
        Assert.Equal("b", (await asB.Notes.SingleAsync()).Text);
    }

    [Fact]
    public async Task A_request_changes_and_deletes_its_own_orgs_rows()
    {
        var store = Guid.NewGuid().ToString();
        await Seed(store, OrgA, "a");
        await using var asA = new NotesDb(store, new FixedTenant(OrgA));
        var note = await asA.Notes.SingleAsync();

        note.Edit("edited");
        await asA.SaveChangesAsync();
        asA.Notes.Remove(note);
        await asA.SaveChangesAsync();

        Assert.Equal(0, await CountAll(store));
    }

    [Fact]
    public async Task The_system_scope_changes_rows_it_loaded_across_the_filter_and_inserts_only_named_ones()
    {
        var store = Guid.NewGuid().ToString();
        await Seed(store, OrgA, "a");
        await Seed(store, OrgB, "b");
        await using var system = new NotesDb(store, FixedTenant.System);

        Assert.Empty(await system.Notes.ToListAsync());
        foreach (var note in await system.Notes.IgnoreQueryFilters().ToListAsync())
            note.Edit(note.Text + "!");
        system.Notes.Add(Note.WriteIn(OrgB, "named"));
        await system.SaveChangesAsync();

        await using var asB = new NotesDb(store, new FixedTenant(OrgB));
        Assert.Equal(["b!", "named"], (await asB.Notes.Select(n => n.Text).ToListAsync()).Order());
        system.Notes.Add(Note.Write("unnamed"));
        await Assert.ThrowsAsync<InvalidOperationException>(() => system.SaveChangesAsync());
    }

    [Fact]
    public void The_filter_must_be_rooted_at_the_context_itself()
    {
        var model = new ModelBuilder();

        Assert.Throws<ArgumentException>(() => model.ApplyTenantFilters(new NotAContext()));
    }

    // Returns the stored row, detached: the context that wrote it is gone.
    private static async Task<Note> Seed(string store, Guid org, string text)
    {
        await using var db = new NotesDb(store, new FixedTenant(org));
        var note = Note.Write(text);
        db.Notes.Add(note);
        await db.SaveChangesAsync();
        return note;
    }

    private static async Task<int> CountAll(string store)
    {
        await using var db = new NotesDb(store, FixedTenant.System);
        return await db.Notes.IgnoreQueryFilters().CountAsync();
    }

    private sealed class NotAContext : ITenantDbContext
    {
        public Guid CurrentOrgId => Guid.Empty;
    }
}

public sealed class Note : ITenantScoped
{
    public Guid Id { get; private set; }

    public Guid OrgId { get; private set; }

    public string Text { get; private set; } = "";

    private Note() { }

    public static Note Write(string text) => new() { Id = Guid.NewGuid(), Text = text };

    public static Note WriteIn(Guid org, string text) => new() { Id = Guid.NewGuid(), OrgId = org, Text = text };

    public void Edit(string text) => Text = text;
}

public sealed class NotesDb(string store, ITenant tenant) : DbContext, ITenantDbContext
{
    public Guid CurrentOrgId => tenant.OrgId;

    public DbSet<Note> Notes => Set<Note>();

    protected override void OnConfiguring(DbContextOptionsBuilder options) =>
        options.UseInMemoryDatabase(store).AddInterceptors(TenantStamping.Instance);

    protected override void OnModelCreating(ModelBuilder model) => model.ApplyTenantFilters(this);
}
