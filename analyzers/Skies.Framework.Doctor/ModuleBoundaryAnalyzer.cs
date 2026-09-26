using System.Collections.Immutable;
using System.Linq;
using System.Threading;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0009 — write-ownership across modules. In the modular monolith every module shares one
/// <c>DbContext</c>, so reading or joining another module's tables is fine (a dashboard is one query). What
/// stays forbidden is <em>writing</em> another module's entity: a slice may only <c>Add</c>/<c>Update</c>/
/// <c>Remove</c> entities of its own module. To affect another module, call its service (in-process) or
/// enqueue a job — never mutate its tables. This is the cheap discipline that keeps a context carvable into
/// its own service later: its writes are already self-contained, so an extraction re-platforms only the
/// cross-module <em>reads</em>, never the writes.
///
/// The check is semantic: a write call (Add / AddRange / Update / UpdateRange / Remove / RemoveRange /
/// Attach / AttachRange and the <c>*Async</c> forms) on a <c>DbSet&lt;T&gt;</c> — or the untyped
/// <c>DbContext.Add(entity)</c> form, which would otherwise be the trivial bypass — whose entity lives under
/// a different <c>Modules.&lt;X&gt;</c> than the calling type. Reads (Where / Any / First / …), joins, and
/// cross-module service calls are not flagged. <c>.Tests.cs</c> files are exempt — a test legitimately seeds
/// any module's data.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class ModuleBoundaryAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for a write into another module's entity.</summary>
    public const string DiagnosticId = "SKY0009";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "A module must write only its own entities",
        messageFormat: "Module '{0}' writes module '{1}'s entity '{2}' — a module owns its writes; call "
                     + "{1}'s service or enqueue a job instead of mutating its tables (reads/joins are fine)",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "In the modular monolith, reads/joins across modules are free but writes are owned: a "
                   + "slice may only Add/Update/Remove its own module's entities. Cross-module effects go "
                   + "through the owning module's service or a job, which keeps each context carvable later.");

    private static readonly ImmutableHashSet<string> WriteMethods = ImmutableHashSet.Create(
        "Add", "AddAsync", "AddRange", "AddRangeAsync",
        "Update", "UpdateRange", "Remove", "RemoveRange", "Attach", "AttachRange");

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterSyntaxNodeAction(AnalyzeInvocation, SyntaxKind.InvocationExpression);
    }

    private static void AnalyzeInvocation(SyntaxNodeAnalysisContext context)
    {
        var invocation = (InvocationExpressionSyntax)context.Node;
        if (invocation.Expression is not MemberAccessExpressionSyntax member)
            return;
        if (!WriteMethods.Contains(member.Name.Identifier.Text))
            return;
        if (IsTestFile(invocation.SyntaxTree.FilePath))
            return;   // a test legitimately seeds any module's data

        // The receiver is a DbSet<TEntity> (the entity is the type argument) or the DbContext itself (the
        // untyped Add(entity) form — the entity is each argument's type). Anything else is not a store write.
        if (context.SemanticModel.GetTypeInfo(member.Expression, context.CancellationToken).Type is not INamedTypeSymbol receiver)
            return;

        foreach (var entity in WrittenEntities(receiver, invocation, context))
        {
            var entityModule = ModuleOf(entity.ContainingNamespace?.ToDisplayString());
            if (entityModule is null)
                continue;   // the entity is not under Modules.<X> (kernel / shared) — nothing to own

            var callerModule = ModuleOf(EnclosingNamespace(context.SemanticModel, invocation, context.CancellationToken));
            if (callerModule is null || callerModule == entityModule)
                continue;   // caller is the composition root / kernel, or it is writing its own module

            context.ReportDiagnostic(Diagnostic.Create(
                Rule, member.Name.GetLocation(), callerModule, entityModule, entity.Name));
        }
    }

    // The entity type(s) a write call touches. On a DbSet<T> the set's type argument; on a DbContext the
    // type of each argument, unwrapped through one array/collection layer (AddRange takes either).
    private static ImmutableArray<ITypeSymbol> WrittenEntities(
        INamedTypeSymbol receiver, InvocationExpressionSyntax invocation, SyntaxNodeAnalysisContext context)
    {
        if (receiver.OriginalDefinition is { Name: "DbSet", TypeArguments.Length: 1 } set
            && set.ContainingNamespace?.ToDisplayString() == "Microsoft.EntityFrameworkCore")
            return ImmutableArray.Create(receiver.TypeArguments[0]);

        if (!DerivesFromDbContext(receiver))
            return ImmutableArray<ITypeSymbol>.Empty;

        return invocation.ArgumentList.Arguments
            .Select(arg => context.SemanticModel.GetTypeInfo(arg.Expression, context.CancellationToken).Type)
            .Where(type => type is not null)
            .Select(type => Unwrap(type!))
            .ToImmutableArray();
    }

    private static bool DerivesFromDbContext(INamedTypeSymbol type)
    {
        for (var t = (INamedTypeSymbol?)type; t is not null; t = t.BaseType)
            if (t.Name == "DbContext"
                && t.ContainingNamespace?.ToDisplayString() == "Microsoft.EntityFrameworkCore")
                return true;
        return false;
    }

    private static ITypeSymbol Unwrap(ITypeSymbol type) => type switch
    {
        IArrayTypeSymbol array => array.ElementType,
        INamedTypeSymbol { TypeArguments.Length: 1 } collection
            when collection.OriginalDefinition.AllInterfaces.Concat([collection.OriginalDefinition])
                .Any(i => i.Name == "IEnumerable") => collection.TypeArguments[0],
        _ => type,
    };

    // The module segment of a namespace like "App.Api.Modules.Account[.Slices]" → "Account"; null when the
    // namespace is not under a Modules.<X> root.
    private static string? ModuleOf(string? ns)
    {
        const string marker = ".Modules.";
        var at = ns?.IndexOf(marker, System.StringComparison.Ordinal) ?? -1;
        if (at < 0)
            return null;
        var rest = ns!.Substring(at + marker.Length);
        var dot = rest.IndexOf('.');
        return dot < 0 ? rest : rest.Substring(0, dot);
    }

    // A test file under the framework's <Concern>.Tests.cs convention.
    private static bool IsTestFile(string? path) =>
        path is not null && path.EndsWith(".Tests.cs", System.StringComparison.Ordinal);

    // The namespace of the type that lexically contains the invocation (the calling slice).
    private static string? EnclosingNamespace(SemanticModel model, SyntaxNode node, CancellationToken ct)
    {
        for (var n = node; n is not null; n = n.Parent)
            if (n is BaseTypeDeclarationSyntax type)
                return model.GetDeclaredSymbol(type, ct)?.ContainingNamespace?.ToDisplayString();
        return null;
    }
}
