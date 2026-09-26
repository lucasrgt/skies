using System.Collections.Immutable;
using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0027 — <b>a slice must not materialize an unbounded set</b>. A <c>ToListAsync</c>/<c>ToList</c> (or the
/// array twins) at the end of a <c>DbSet</c>-rooted chain with no <c>Take</c> and no <c>ToPageAsync</c> on the
/// way serves a whole table as one response. The defect is invisible in development — ten rows always come back
/// fast — and degrades in production as the tenant's data grows, months later. Warning tier on purpose:
/// legitimately small sets exist (lookup tables, the lines of one order), and the fix is also the
/// documentation — <c>.Take(n)</c> writes the bound down, <c>ToPageAsync</c> pages it.
///
/// A <b>parent-scoped</b> query is exempt: a <c>Where</c> equating (or <c>Contains</c>-matching) a member
/// whose name ends in <c>Id</c> — <c>s =&gt; s.JobId == id</c>, the steps of ONE job, the sessions of ONE
/// user — is bounded by the parent aggregate's cardinality, not by the tenant's growth, and a synthetic
/// <c>.Take(n)</c> there would document a bound that isn't the real rule (the pauta 0.3.0 adoption surfaced
/// ~16 such sites). <c>OrgId</c>/<c>TenantId</c> equality is NOT parent scoping — that is exactly the
/// grows-with-the-tenant set the rule exists for — so those two names stay flagged.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class UnboundedMaterializationAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported when a slice materializes a DbSet-rooted query with no bound.</summary>
    public const string DiagnosticId = "SKY0027";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "A slice must not materialize an unbounded set",
        messageFormat: "{0}",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Warning,
        isEnabledByDefault: true,
        description: "A list served straight off a DbSet grows with the data: fine with ten development rows, "
                   + "a multi-second response and a memory spike once a tenant has fifty thousand. ToPageAsync "
                   + "pages it behind a stable order; an explicit Take(n) documents why the set is small.");

    private static readonly ImmutableHashSet<string> Materializers =
        ImmutableHashSet.Create("ToListAsync", "ToArrayAsync", "ToList", "ToArray");

    // GroupBy is a bound of its own kind: what materializes is the aggregated groups, not the table's rows —
    // the dashboard rollup shape (counts per status), not the serve-the-whole-list defect.
    private static readonly ImmutableHashSet<string> Bounds = ImmutableHashSet.Create("Take", "ToPageAsync", "GroupBy");

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterSyntaxNodeAction(AnalyzeInvocation, Microsoft.CodeAnalysis.CSharp.SyntaxKind.InvocationExpression);
    }

    private static void AnalyzeInvocation(SyntaxNodeAnalysisContext context)
    {
        var invocation = (InvocationExpressionSyntax)context.Node;
        if (invocation.Expression is not MemberAccessExpressionSyntax member
            || !Materializers.Contains(member.Name.Identifier.Text)
            || !InsideSlice(invocation))
            return;

        // Walk the fluent chain from the materializer back to its root. A Take/ToPageAsync on the way is
        // the bound; a receiver typed DbSet on the way is the unbounded root.
        var expression = member.Expression;
        var rootsInDbSet = false;
        while (true)
        {
            if (IsDbSet(context.SemanticModel.GetTypeInfo(expression).Type))
            {
                rootsInDbSet = true;
                break;
            }
            if (expression is InvocationExpressionSyntax { Expression: MemberAccessExpressionSyntax inner } step)
            {
                if (Bounds.Contains(inner.Name.Identifier.Text))
                {
                    if (inner.Name.Identifier.Text == "Take"
                        && step.ArgumentList.Arguments.FirstOrDefault()?.Expression is LiteralExpressionSyntax literal
                        && literal.IsKind(Microsoft.CodeAnalysis.CSharp.SyntaxKind.NumericLiteralExpression))
                        // A bare number is how a guess ("500 is surely enough") passes for a decision and later
                        // truncates silently; the ladder asks for a named bound.
                        context.ReportDiagnostic(Diagnostic.Create(Rule, literal.GetLocation(),
                            $"'Take({literal.Token.ValueText})' bounds the query with a bare number — name the bound (a const "
                            + "such as MaxQueue, with a comment saying why the set is that small) or page it with ToPageAsync"));
                    return;
                }
                if (inner.Name.Identifier.Text == "Where" && step.ArgumentList.Arguments.Any(a => ParentScopes(a.Expression)))
                    return;
                expression = inner.Expression;
                continue;
            }
            break;
        }

        // A chain rooted in a local (the `query = db.Points.AsQueryable(); query = query.Where(...)` shape):
        // the bound may live in any assignment to that local, and only a DbSet somewhere in those
        // assignments makes the local a database query rather than an in-memory AsQueryable.
        if (!rootsInDbSet)
        {
            if (expression is not IdentifierNameSyntax local || !IsQueryableName(context.SemanticModel.GetTypeInfo(local).Type))
                return;
            var sources = AssignmentsTo(invocation, local.Identifier.Text);
            if (sources.Count == 0 || sources.Any(ContainsBound))
                return;
            if (!sources.Any(source => source.DescendantNodesAndSelf().OfType<ExpressionSyntax>()
                    .Any(node => IsDbSet(context.SemanticModel.GetTypeInfo(node).Type))))
                return;
        }

        context.ReportDiagnostic(Diagnostic.Create(Rule, member.Name.GetLocation(),
            $"'{member.Name.Identifier.Text}' materializes a DbSet-rooted query with no bound — page it with ToPageAsync "
            + "or write the bound down with a named Take"));
    }

    private static bool InsideSlice(SyntaxNode node) =>
        node.Ancestors().OfType<TypeDeclarationSyntax>().Any(type =>
            type.AttributeLists.SelectMany(list => list.Attributes)
                .Any(attribute => NameOf(attribute) is "Slice" or "SliceAttribute"));

    private static string NameOf(AttributeSyntax attribute) => attribute.Name switch
    {
        QualifiedNameSyntax qualified => qualified.Right.Identifier.Text,
        SimpleNameSyntax simple => simple.Identifier.Text,
        _ => "",
    };

    private static bool IsDbSet(ITypeSymbol? type) => type?.Name == "DbSet";

    private static bool IsQueryableName(ITypeSymbol? type) =>
        type?.Name is "IQueryable" or "IOrderedQueryable" or "DbSet";

    // Every expression assigned to `name` in the enclosing method — the declarator's initializer and any
    // re-assignment — so the bound is honored wherever the query was narrowed.
    private static System.Collections.Generic.List<ExpressionSyntax> AssignmentsTo(SyntaxNode from, string name)
    {
        var method = from.FirstAncestorOrSelf<MethodDeclarationSyntax>();
        if (method is null)
            return [];
        var sources = new System.Collections.Generic.List<ExpressionSyntax>();
        foreach (var node in method.DescendantNodes())
            switch (node)
            {
                case VariableDeclaratorSyntax declarator
                    when declarator.Identifier.Text == name && declarator.Initializer is { } initializer:
                    sources.Add(initializer.Value);
                    break;
                case AssignmentExpressionSyntax { Left: IdentifierNameSyntax left } assignment
                    when left.Identifier.Text == name:
                    sources.Add(assignment.Right);
                    break;
            }
        return sources;
    }

    private static bool ContainsBound(ExpressionSyntax expression) =>
        expression.DescendantNodesAndSelf().OfType<InvocationExpressionSyntax>()
            .Any(invocation => invocation.Expression is MemberAccessExpressionSyntax member
                && (Bounds.Contains(member.Name.Identifier.Text)
                    || (member.Name.Identifier.Text == "Where"
                        && invocation.ArgumentList.Arguments.Any(a => ParentScopes(a.Expression)))));

    // A Where predicate that pins the set to one parent aggregate: an equality (or a Contains match) on a
    // *Id member OF THE QUERIED ENTITY — the member access must root in the lambda's parameter, so the value
    // side of the comparison (input.AgencyId, caller.UserId) never grants the exemption by itself.
    // OrgId/TenantId are the tenant scope, the set the rule is about — never a parent.
    private static bool ParentScopes(ExpressionSyntax predicate)
    {
        var parameter = predicate switch
        {
            SimpleLambdaExpressionSyntax simple => simple.Parameter.Identifier.Text,
            ParenthesizedLambdaExpressionSyntax { ParameterList.Parameters: { Count: > 0 } parameters } =>
                parameters[0].Identifier.Text,
            _ => null,
        };
        if (parameter is null)
            return false;

        return predicate.DescendantNodesAndSelf().Any(node => node switch
        {
            BinaryExpressionSyntax { RawKind: (int)Microsoft.CodeAnalysis.CSharp.SyntaxKind.EqualsExpression } eq =>
                IsParentKey(eq.Left, parameter) || IsParentKey(eq.Right, parameter),
            InvocationExpressionSyntax { Expression: MemberAccessExpressionSyntax { Name.Identifier.Text: "Contains" } } contains =>
                contains.ArgumentList.Arguments.Any(a => IsParentKey(a.Expression, parameter)),
            _ => false,
        });
    }

    private static bool IsParentKey(ExpressionSyntax side, string parameter) =>
        side is MemberAccessExpressionSyntax { Name.Identifier.Text: var name, Expression: IdentifierNameSyntax root }
        && root.Identifier.Text == parameter
        && name.EndsWith("Id", System.StringComparison.Ordinal)
        && name is not ("OrgId" or "TenantId");
}
