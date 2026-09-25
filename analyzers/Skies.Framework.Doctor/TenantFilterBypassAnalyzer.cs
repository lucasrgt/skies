using System.Collections.Generic;
using System.Collections.Immutable;
using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0030 — <b>app code never crosses the tenant boundary unannounced</b>. Two calls step outside the org a request
/// acts in: <c>IgnoreQueryFilters()</c>, which lifts the query filter that scopes every <c>ITenantScoped</c> entity to
/// the caller's org (a cross-org read that compiles and passes review at a glance), and a <c>FixedTenant</c>, which
/// hands a context an org (or the system scope) of the code's choosing instead of the request's. Both are flagged
/// anywhere in the app's own code; specs and tests (a <c>.specs</c> path segment, a <c>Specs.*</c> namespace, a
/// co-located <c>*.Tests.cs</c>) cross orgs on purpose and are out of scope. Lifting only an app's own named filter
/// (<c>IgnoreQueryFilters(["soft-delete"])</c>) is not a crossing and stays legal.
/// <para>The legitimate crossings (sign-in looks a user up by email before any org is known; a nightly job runs as
/// the system) take the usual hatch, with the reason required: <c>#pragma warning disable SKY0030 // &lt;why&gt;</c>, or
/// <c>[SuppressMessage(..., "SKY0030", Justification = "&lt;why&gt;")]</c>. A hatch without its reason is itself
/// reported, and so is a bare <c>#pragma warning disable</c> (no rule named) in a file that crosses, so every
/// crossing is visible and explained where it happens.</para>
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

    private const string ChosenTenant =
        "'FixedTenant' gives this context an org (or the system scope) of the code's choosing, not the request's. "
        + "Take the request's ITenant, or, for a deliberate crossing (a background job), suppress SKY0030 here with the reason";

    private const string NoReason =
        "This '#pragma warning disable SKY0030' gives no reason. Write why the code crosses orgs after it: "
        + "'#pragma warning disable SKY0030 // <reason>'";

    private const string BarePragma =
        "This '#pragma warning disable' names no rule, so it also hides SKY0030 on the tenant crossing below. Name the "
        + "rules it disables, with the reason: '#pragma warning disable SKY0030 // <reason>'";

    private const string NoJustification =
        "A [SuppressMessage] of SKY0030 in this file gives no Justification. Say why the code crosses orgs: "
        + "Justification = \"<reason>\"";

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
        context.RegisterSyntaxNodeAction(AnalyzeFixedTenant, SyntaxKind.ObjectCreationExpression, SyntaxKind.SimpleMemberAccessExpression);
        context.RegisterSyntaxTreeAction(AnalyzePragmas);
        context.RegisterSyntaxTreeAction(AnalyzeSuppressMessages);
    }

    private static void AnalyzeInvocation(SyntaxNodeAnalysisContext context)
    {
        var invocation = (InvocationExpressionSyntax)context.Node;
        if (invocation.Expression is not MemberAccessExpressionSyntax { Name.Identifier.Text: "IgnoreQueryFilters" } member)
            return;
        if (!InAppCode(invocation) || !MayLiftTenantFilter(invocation.ArgumentList))
            return;
        context.ReportDiagnostic(Diagnostic.Create(Rule, member.Name.GetLocation(), Crossing));
    }

    // `new FixedTenant(org)` and `FixedTenant.System`: an org the code picks. Matched by name, like the EF call.
    private static void AnalyzeFixedTenant(SyntaxNodeAnalysisContext context)
    {
        var at = context.Node switch
        {
            ObjectCreationExpressionSyntax { Type: var type } when SimpleName(type) == "FixedTenant" => type.GetLocation(),
            MemberAccessExpressionSyntax { Expression: var target, Name.Identifier.Text: "System" } access
                when SimpleName(target) == "FixedTenant" => access.GetLocation(),
            _ => null,
        };
        if (at is not null && InAppCode(context.Node))
            context.ReportDiagnostic(Diagnostic.Create(Rule, at, ChosenTenant));
    }

    private static string SimpleName(SyntaxNode node) => node switch
    {
        QualifiedNameSyntax qualified => qualified.Right.Identifier.Text,
        MemberAccessExpressionSyntax access => access.Name.Identifier.Text,
        SimpleNameSyntax simple => simple.Identifier.Text,
        _ => "",
    };

    // A reason is the comment after the codes on the directive's own line. The finding sits on the directive, before
    // the state it sets takes effect, so the reasonless pragma cannot hide its own report.
    private static void AnalyzePragmas(SyntaxTreeAnalysisContext context)
    {
        var root = context.Tree.GetRoot(context.CancellationToken);
        foreach (var trivia in root.DescendantTrivia(descendIntoTrivia: true))
        {
            if (trivia.GetStructure() is not PragmaWarningDirectiveTriviaSyntax pragma
                || !pragma.DisableOrRestoreKeyword.IsKind(SyntaxKind.DisableKeyword))
                continue;
            if (pragma.ErrorCodes.Count == 0)
            {
                // A blanket disable hides every rule after it, SKY0030 included, and says nothing about why.
                if (CrossesAfter(root, pragma.SpanStart))
                    context.ReportDiagnostic(Diagnostic.Create(Rule, pragma.GetLocation(), BarePragma));
                continue;
            }
            if (!pragma.ErrorCodes.Any(code => code.ToString().Trim() == DiagnosticId))
                continue;
            var text = pragma.ToFullString();
            var comment = text.IndexOf("//", System.StringComparison.Ordinal);
            if (comment < 0 || text.Substring(comment + 2).Trim().Length == 0)
                context.ReportDiagnostic(Diagnostic.Create(Rule, pragma.GetLocation(), NoReason));
        }
    }

    // A [SuppressMessage] silences its whole target, so its report goes where the target cannot reach it: the file's
    // namespace name, or its first token when it has none. The message names the attribute's line.
    private static void AnalyzeSuppressMessages(SyntaxTreeAnalysisContext context)
    {
        var root = context.Tree.GetRoot(context.CancellationToken);
        foreach (var attribute in root.DescendantNodes().OfType<AttributeSyntax>())
        {
            var name = attribute.Name.ToString();
            var simple = name.Substring(name.LastIndexOf('.') + 1);
            if (simple is not ("SuppressMessage" or "SuppressMessageAttribute")
                || attribute.ArgumentList is not { Arguments: { Count: >= 2 } arguments}
                || !(arguments[1].Expression is LiteralExpressionSyntax literal
                     && literal.Token.ValueText.StartsWith(DiagnosticId, System.StringComparison.Ordinal)))
                continue;
            var justified = arguments.Any(argument =>
                argument.NameEquals?.Name.Identifier.Text == "Justification"
                && !(argument.Expression is LiteralExpressionSyntax { Token.ValueText: var reason }
                     && string.IsNullOrWhiteSpace(reason)));
            if (justified)
                continue;
            var anchor = root.DescendantNodes().OfType<BaseNamespaceDeclarationSyntax>().FirstOrDefault()?.Name.GetLocation()
                ?? root.GetFirstToken().GetLocation();
            var line = attribute.GetLocation().GetLineSpan().StartLinePosition.Line + 1;
            context.ReportDiagnostic(Diagnostic.Create(Rule, anchor, $"{NoJustification} (line {line})"));
        }
    }

    // Whether the file crosses the tenant boundary after a position: the calls this rule reports.
    private static bool CrossesAfter(SyntaxNode root, int position) =>
        root.DescendantNodes().Any(node => node.SpanStart > position && InAppCode(node) && node switch
        {
            InvocationExpressionSyntax { Expression: MemberAccessExpressionSyntax { Name.Identifier.Text: "IgnoreQueryFilters" } } invocation
                => MayLiftTenantFilter(invocation.ArgumentList),
            ObjectCreationExpressionSyntax { Type: var type } => SimpleName(type) == "FixedTenant",
            MemberAccessExpressionSyntax { Expression: var target, Name.Identifier.Text: "System" } => SimpleName(target) == "FixedTenant",
            _ => false,
        });

    // The app's own code: everything but specs and tests (a `.specs` path segment, a `Specs.*` namespace, a
    // co-located `*.Tests.cs`), which assert across orgs on purpose.
    private static bool InAppCode(SyntaxNode node)
    {
        var path = node.SyntaxTree.FilePath ?? "";
        if (path.EndsWith(".Tests.cs", System.StringComparison.OrdinalIgnoreCase)
            || path.Replace('\\', '/').Split('/').Contains(".specs"))
            return false;
        return !node.Ancestors().OfType<BaseNamespaceDeclarationSyntax>()
            .Any(ns => ns.Name.ToString() == "Specs" || ns.Name.ToString().StartsWith("Specs.", System.StringComparison.Ordinal));
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
