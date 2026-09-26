using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.Immutable;
using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0006 — no repository / unit-of-work abstraction over the <c>DbContext</c>. A Skies slice queries the
/// <c>DbContext</c> directly in its <c>Handle</c>; the repository / UoW layer of clean architecture is indirection the
/// framework deliberately cuts. The anti-pattern is a type named for that layer (<c>*Repository</c>,
/// <c>*UnitOfWork</c>) that <em>wraps the EF Core context</em>: it holds a <c>DbContext</c> or a <c>DbSet&lt;T&gt;</c>
/// in a field, a property, or a constructor parameter. Such a class is flagged at its declaration, and so is every
/// source interface named for the layer that it implements, so the diagnostic points at each place to delete.
///
/// A type that merely carries the name is not the layer: a vendor client (<c>GitHubRepository</c> over an
/// <c>HttpClient</c>), a package-registry model, or an in-memory lookup wraps no context and is left alone. EF Core's
/// own <c>DbSet</c>/<c>DbContext</c> are the data door, not a repository, and never match.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class NoRepositoryAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for a repository/unit-of-work abstraction over the DbContext.</summary>
    public const string DiagnosticId = "SKY0006";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "No repository / unit-of-work abstraction over the DbContext",
        messageFormat: "'{0}' wraps the DbContext in a repository/unit-of-work layer — a Skies slice reads the "
                     + "DbContext directly in Handle; delete the abstraction",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "Slices query the DbContext directly; the repository/UoW layer of clean architecture is "
                   + "indirection the framework cuts. A *Repository or *UnitOfWork type that holds a DbContext or a "
                   + "DbSet (field, property, or constructor parameter), and the source interfaces it implements "
                   + "under those names, reintroduce it. A type that only carries the name is not flagged.");

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterCompilationStartAction(start =>
        {
            // An interface can be implemented by several wrappers; report it once.
            var reported = new ConcurrentDictionary<ISymbol, bool>(SymbolEqualityComparer.Default);
            start.RegisterSymbolAction(symbol => Analyze(symbol, reported), SymbolKind.NamedType);
        });
    }

    private static void Analyze(SymbolAnalysisContext context, ConcurrentDictionary<ISymbol, bool> reported)
    {
        var type = (INamedTypeSymbol)context.Symbol;
        if (type.TypeKind is not (TypeKind.Class or TypeKind.Struct) || !IsLayerName(type.Name) || !WrapsContext(type))
            return;

        Report(context, type, reported);
        foreach (var contract in type.AllInterfaces.Where(i => IsLayerName(i.Name) && i.Locations.Any(l => l.IsInSource)))
            Report(context, contract, reported);
    }

    private static void Report(SymbolAnalysisContext context, INamedTypeSymbol type, ConcurrentDictionary<ISymbol, bool> reported)
    {
        if (!reported.TryAdd(type, true))
            return;
        foreach (var location in type.Locations.Where(l => l.IsInSource))
            context.ReportDiagnostic(Diagnostic.Create(Rule, location, type.Name));
    }

    // The layer's name: `*Repository` (OrderRepository, IRepository) or `*UnitOfWork` (IUnitOfWork).
    private static bool IsLayerName(string name) =>
        name.EndsWith("Repository", StringComparison.Ordinal) || name.EndsWith("UnitOfWork", StringComparison.Ordinal);

    // Whether the type holds the EF Core context: a field, a property, or a constructor parameter (primary
    // constructors included) typed as a DbContext or a DbSet<T>.
    private static bool WrapsContext(INamedTypeSymbol type)
    {
        var held = type.GetMembers().SelectMany(member => member switch
        {
            IFieldSymbol field => new[] { field.Type },
            IPropertySymbol property => new[] { property.Type },
            IMethodSymbol { MethodKind: MethodKind.Constructor } ctor => ctor.Parameters.Select(p => p.Type),
            _ => Enumerable.Empty<ITypeSymbol>(),
        });
        return held.Any(IsContext);
    }

    private static bool IsContext(ITypeSymbol type)
    {
        if (type is INamedTypeSymbol { Name: "DbSet", IsGenericType: true })
            return true;
        for (var current = type; current is not null; current = current.BaseType)
            if (current.Name == "DbContext")
                return true;
        return false;
    }
}
