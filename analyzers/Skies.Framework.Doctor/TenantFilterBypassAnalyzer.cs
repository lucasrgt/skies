using System.Collections.Generic;
using System.Collections.Immutable;
using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0030 — <b>a module never reads across the tenant filter unannounced</b>. <c>IgnoreQueryFilters()</c> lifts the
/// query filter that scopes every <c>ITenantScoped</c> entity to the caller's org, so in a slice or module it is a
/// cross-org read that compiles, passes review at a glance, and was doctor-clean. It is flagged in module code (a
/// namespace with a <c>Modules</c> segment, or a <c>[Slice]</c>/<c>[Module]</c> class) whenever it may lift the tenant
/// filter: with no argument, or with filter names that are not all literals other than <c>"tenant"</c>. Lifting only
/// an app's own named filter (<c>IgnoreQueryFilters(["soft-delete"])</c>) is not a cross-org read and stays legal.
/// <para>The legitimate crossings (sign-in and uniqueness look a user up by email before any org is known) take the
/// usual hatch, with the reason required: <c>#pragma warning disable SKY0030 // &lt;why this read crosses orgs&gt;</c>.
/// A <c>#pragma</c> that disables SKY0030 with no reason beside it is itself reported, so every crossing is visible
/// and explained where it happens.</para>
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class TenantFilterBypassAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for a cross-org read, and for its suppression without a reason.</summary>
    public const string DiagnosticId = "SKY0030";

    private const string TenantFilterName = "tenant";

    private const string Crossing =
        "'IgnoreQueryFilters' lifts the tenant filter, so this query reads every org's rows. Scope the query to the "
        + "caller's org, or, for a deliberate crossing (sign-in by email), suppress SKY0030 here with the reason";

    private const string NoReason =
        "This '#pragma warning disable SKY0030' gives no reason. Write why the read crosses orgs after it: "
        + "'#pragma warning disable SKY0030 // <reason>'";

    // One descriptor, two messages: the crossing, and a suppression of it that does not say why.
    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "A module must not lift the tenant filter unannounced",
        messageFormat: "{0}",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "IgnoreQueryFilters() in a slice or module lifts the tenant query filter and reads other orgs' "
                   + "rows. Every such crossing is a reviewed exception, suppressed where it happens with its reason: "
                   + "'#pragma warning disable SKY0030 // <reason>'.");

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterSyntaxNodeAction(AnalyzeInvocation, SyntaxKind.InvocationExpression);
        context.RegisterSyntaxTreeAction(AnalyzePragmas);
    }

    private static void AnalyzeInvocation(SyntaxNodeAnalysisContext context)
    {
        var invocation = (InvocationExpressionSyntax)context.Node;
        if (invocation.Expression is not MemberAccessExpressionSyntax { Name.Identifier.Text: "IgnoreQueryFilters" } member)
            return;
        if (!InModuleCode(invocation) || !MayLiftTenantFilter(invocation.ArgumentList))
            return;
        context.ReportDiagnostic(Diagnostic.Create(Rule, member.Name.GetLocation(), Crossing));
    }

    // A reason is the comment after the codes on the directive's own line. The finding sits on the directive, before
    // the state it sets takes effect, so the reasonless pragma cannot hide its own report.
    private static void AnalyzePragmas(SyntaxTreeAnalysisContext context)
    {
        var root = context.Tree.GetRoot(context.CancellationToken);
        foreach (var trivia in root.DescendantTrivia(descendIntoTrivia: true))
        {
            if (trivia.GetStructure() is not PragmaWarningDirectiveTriviaSyntax pragma
                || !pragma.DisableOrRestoreKeyword.IsKind(SyntaxKind.DisableKeyword)
                || !pragma.ErrorCodes.Any(code => code.ToString().Trim() == DiagnosticId))
                continue;
            var text = pragma.ToFullString();
            var comment = text.IndexOf("//", System.StringComparison.Ordinal);
            if (comment < 0 || text.Substring(comment + 2).Trim().Length == 0)
                context.ReportDiagnostic(Diagnostic.Create(Rule, pragma.GetLocation(), NoReason));
        }
    }

    // Module code: under a namespace with a `Modules` segment (App.Api.Modules.Billing), or inside a class marked
    // [Slice] or [Module]. Specs and tests (a `.specs` path segment, a co-located `*.Tests.cs`) assert across orgs on
    // purpose, and the platform's own composition is not a module, so they are out of scope.
    private static bool InModuleCode(SyntaxNode node)
    {
        var path = node.SyntaxTree.FilePath ?? "";
        if (path.EndsWith(".Tests.cs", System.StringComparison.OrdinalIgnoreCase)
            || path.Replace('\\', '/').Split('/').Contains(".specs"))
            return false;
        var inModulesNamespace = node.Ancestors().OfType<BaseNamespaceDeclarationSyntax>()
            .Any(ns => ns.Name.ToString().Split('.').Contains("Modules"));
        return inModulesNamespace || node.Ancestors().OfType<ClassDeclarationSyntax>()
            .Any(cls => cls.AttributeLists.SelectMany(list => list.Attributes).Any(IsSliceOrModule));
    }

    private static bool IsSliceOrModule(AttributeSyntax attribute)
    {
        var name = attribute.Name.ToString();
        var simple = name.Substring(name.LastIndexOf('.') + 1);
        return simple is "Slice" or "SliceAttribute" or "Module" or "ModuleAttribute";
    }

    // Lifting only named filters that are all string literals other than "tenant" keeps the tenant filter on.
    private static bool MayLiftTenantFilter(ArgumentListSyntax arguments)
    {
        if (arguments.Arguments.Count == 0)
            return true;
        var names = arguments.Arguments.SelectMany(arg => FilterNames(arg.Expression)).ToList();
        return names.Count == 0 || names.Any(name => name is null || name == TenantFilterName);
    }

    // The literal filter names an argument spells, or a null entry for anything that is not a plain literal.
    private static IEnumerable<string?> FilterNames(ExpressionSyntax expression) => expression switch
    {
        LiteralExpressionSyntax { RawKind: (int)SyntaxKind.StringLiteralExpression } literal =>
            new[] { literal.Token.ValueText },
        CollectionExpressionSyntax collection =>
            collection.Elements.SelectMany(e => e is ExpressionElementSyntax element
                ? FilterNames(element.Expression)
                : new string?[] { null }),
        ArrayCreationExpressionSyntax { Initializer: { } init } => init.Expressions.SelectMany(FilterNames),
        ImplicitArrayCreationExpressionSyntax implicitArray => implicitArray.Initializer.Expressions.SelectMany(FilterNames),
        _ => new string?[] { null },
    };
}
