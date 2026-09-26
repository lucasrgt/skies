using System.Collections.Generic;
using System.Collections.Immutable;
using System.Linq;
using System.Threading;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0026 — <b>every persisted write declares its concurrency posture</b>. A slice whose
/// <c>Handle</c> mutates a tracked entity that carries no concurrency token runs last-write-wins: two
/// concurrent requests both read the same row, both mutate, and the second save silently erases the first —
/// the classic double-spend. Insert-only rows and entities merely read beside another write are not reported:
/// a concurrency token cannot improve either case. The token is one property:
/// <c>[Timestamp] public byte[]? RowVersion {{ get; private set; }}</c> (or <c>[ConcurrencyCheck]</c> on a
/// domain field), after which a conflicting save surfaces as <c>DbUpdateConcurrencyException</c> instead of
/// silent loss. Warning-tier: a fluent-only configuration the doctor cannot see is legal — name the property
/// <c>RowVersion</c> (recognized). The release gate rejects local suppression of this contract.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class WriteConcurrencyAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported when a write saves an entity that has no concurrency token.</summary>
    public const string DiagnosticId = "SKY0026";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "A persisted write should be guarded by a concurrency token",
        messageFormat: "Write slice '{0}' saves entity '{1}' which declares no concurrency token — "
                     + "concurrent requests are last-write-wins; add a [Timestamp] RowVersion (or "
                     + "[ConcurrencyCheck]) so a conflicting save fails loudly instead of silently losing a write",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Warning,
        isEnabledByDefault: true,
        description: "A slice that mutates a tracked entity with no concurrency token "
                   + "(no [Timestamp]/[ConcurrencyCheck] member, no RowVersion property) runs last-write-wins "
                   + "on the operation. Warning-tier: fluent-only configuration "
                   + "is invisible to the doctor — expose the conventional RowVersion property.");

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterSyntaxNodeAction(AnalyzeClass, SyntaxKind.ClassDeclaration);
    }

    private static void AnalyzeClass(SyntaxNodeAnalysisContext context)
    {
        var cls = (ClassDeclarationSyntax)context.Node;
        if (!SliceBehavior.IsSlice(cls))
            return;

        var handle = cls.Members.OfType<MethodDeclarationSyntax>()
            .FirstOrDefault(m => m.Identifier.Text == "Handle");
        if (handle is null || !SavesChanges(handle))
            return;

        foreach (var entity in WrittenEntities(handle, context))
            if (!HasConcurrencyToken(entity))
                context.ReportDiagnostic(Diagnostic.Create(
                    Rule, cls.Identifier.GetLocation(), cls.Identifier.Text, entity.Name));
    }

    private static bool SavesChanges(MethodDeclarationSyntax handle) =>
        handle.DescendantNodes()
            .OfType<MemberAccessExpressionSyntax>()
            .Any(ma => ma.Name.Identifier.Text is "SaveChanges" or "SaveChangesAsync");

    // A query beside a save is not proof that the queried row was written. Start with the DbSet-backed entity
    // inventory, then retain only receivers whose state is assigned/incremented or explicitly updated/removed.
    // Insert-only Add/AddAsync calls intentionally stay out: optimistic concurrency cannot improve an insert.
    private static IEnumerable<INamedTypeSymbol> WrittenEntities(
        MethodDeclarationSyntax handle, SyntaxNodeAnalysisContext context)
    {
        var touched = new HashSet<INamedTypeSymbol>(
            DbSetEntities(handle, context),
            SymbolEqualityComparer.Default);
        var written = new HashSet<INamedTypeSymbol>(SymbolEqualityComparer.Default);

        foreach (var assignment in handle.DescendantNodes().OfType<AssignmentExpressionSyntax>())
            AddReceiverEntity(assignment.Left, context, touched, written);
        foreach (var mutation in handle.DescendantNodes().OfType<PostfixUnaryExpressionSyntax>())
            AddReceiverEntity(mutation.Operand, context, touched, written);
        foreach (var mutation in handle.DescendantNodes().OfType<PrefixUnaryExpressionSyntax>())
            if (mutation.IsKind(SyntaxKind.PreIncrementExpression)
                || mutation.IsKind(SyntaxKind.PreDecrementExpression))
                AddReceiverEntity(mutation.Operand, context, touched, written);

        foreach (var invocation in handle.DescendantNodes().OfType<InvocationExpressionSyntax>())
        {
            if (invocation.Expression is not MemberAccessExpressionSyntax member)
                continue;
            var receiverType = context.SemanticModel.GetTypeInfo(
                member.Expression,
                context.CancellationToken).Type as INamedTypeSymbol;
            if (receiverType is not null
                && touched.Contains(receiverType)
                && context.SemanticModel.GetSymbolInfo(invocation, context.CancellationToken).Symbol
                    is IMethodSymbol entityMethod
                && MutatesInstance(
                    entityMethod,
                    context.CancellationToken,
                    new HashSet<IMethodSymbol>(SymbolEqualityComparer.Default)))
            {
                written.Add(receiverType);
                continue;
            }

            var method = member.Name.Identifier.Text;
            if (method is not ("Update" or "UpdateRange" or "Remove" or "RemoveRange"
                or "ExecuteUpdate" or "ExecuteUpdateAsync" or "ExecuteDelete" or "ExecuteDeleteAsync"))
                continue;

            if (DbSetEntity(receiverType) is { } setEntity)
            {
                written.Add(setEntity);
                continue;
            }

            foreach (var argument in invocation.ArgumentList.Arguments)
                if (context.SemanticModel.GetTypeInfo(argument.Expression, context.CancellationToken).Type
                        is INamedTypeSymbol entity
                    && touched.Contains(entity))
                    written.Add(entity);
        }

        return written;
    }

    private static bool MutatesInstance(
        IMethodSymbol method,
        CancellationToken cancellationToken,
        HashSet<IMethodSymbol> visited)
    {
        var target = method.OriginalDefinition;
        if (target.IsStatic || !visited.Add(target))
            return false;

        foreach (var reference in target.DeclaringSyntaxReferences)
        {
            var declaration = reference.GetSyntax(cancellationToken);
            var locals = new HashSet<string>(
                declaration.DescendantNodes()
                    .OfType<VariableDeclaratorSyntax>()
                    .Select(variable => variable.Identifier.Text));
            locals.UnionWith(target.Parameters.Select(parameter => parameter.Name));
            locals.UnionWith(declaration.DescendantNodes()
                .OfType<ParameterSyntax>()
                .Select(parameter => parameter.Identifier.Text));
            locals.UnionWith(declaration.DescendantNodes()
                .OfType<ForEachStatementSyntax>()
                .Select(loop => loop.Identifier.Text));
            locals.UnionWith(declaration.DescendantNodes()
                .OfType<CatchDeclarationSyntax>()
                .Select(catchDeclaration => catchDeclaration.Identifier.Text));
            foreach (var assignment in declaration.DescendantNodes().OfType<AssignmentExpressionSyntax>())
                if (TargetsInstance(assignment.Left, locals))
                    return true;
            foreach (var mutation in declaration.DescendantNodes().OfType<PrefixUnaryExpressionSyntax>())
                if ((mutation.IsKind(SyntaxKind.PreIncrementExpression)
                        || mutation.IsKind(SyntaxKind.PreDecrementExpression))
                    && TargetsInstance(mutation.Operand, locals))
                    return true;
            foreach (var mutation in declaration.DescendantNodes().OfType<PostfixUnaryExpressionSyntax>())
                if (TargetsInstance(mutation.Operand, locals))
                    return true;

            foreach (var invocation in declaration.DescendantNodes().OfType<InvocationExpressionSyntax>())
            {
                if (invocation.Expression is MemberAccessExpressionSyntax member
                    && member.Name.Identifier.Text is "Add" or "AddRange" or "Clear" or "Insert"
                        or "Remove" or "RemoveAll"
                    && TargetsInstance(member.Expression, locals))
                    return true;

                var calledName = invocation.Expression switch
                {
                    IdentifierNameSyntax identifier => identifier.Identifier.Text,
                    MemberAccessExpressionSyntax { Expression: ThisExpressionSyntax } directMember =>
                        directMember.Name.Identifier.Text,
                    _ => null,
                };
                if (calledName is null)
                    continue;
                foreach (var called in target.ContainingType.GetMembers(calledName).OfType<IMethodSymbol>())
                    if (MutatesInstance(called, cancellationToken, visited))
                        return true;
            }
        }

        return false;
    }

    private static bool TargetsInstance(ExpressionSyntax expression, HashSet<string> locals)
    {
        while (expression is ParenthesizedExpressionSyntax parenthesized)
            expression = parenthesized.Expression;
        while (expression is MemberAccessExpressionSyntax member)
            expression = member.Expression;
        while (expression is ElementAccessExpressionSyntax element)
            expression = element.Expression;

        return expression is ThisExpressionSyntax
            || expression is IdentifierNameSyntax identifier
            && !locals.Contains(identifier.Identifier.Text);
    }

    private static IEnumerable<INamedTypeSymbol> DbSetEntities(
        MethodDeclarationSyntax handle, SyntaxNodeAnalysisContext context)
    {
        var seen = new HashSet<INamedTypeSymbol>(SymbolEqualityComparer.Default);
        foreach (var expression in handle.DescendantNodes().OfType<MemberAccessExpressionSyntax>())
            if (DbSetEntity(context.SemanticModel.GetTypeInfo(expression, context.CancellationToken).Type)
                    is { } entity
                && seen.Add(entity))
                yield return entity;
    }

    private static INamedTypeSymbol? DbSetEntity(ITypeSymbol? type) =>
        type is INamedTypeSymbol { Name: "DbSet", TypeArguments.Length: 1 } set
        && set.ContainingNamespace?.ToDisplayString() == "Microsoft.EntityFrameworkCore"
        && set.TypeArguments[0] is INamedTypeSymbol entity
            ? entity
            : null;

    private static void AddReceiverEntity(
        ExpressionSyntax expression,
        SyntaxNodeAnalysisContext context,
        HashSet<INamedTypeSymbol> touched,
        HashSet<INamedTypeSymbol> written)
    {
        var root = expression;
        while (root is MemberAccessExpressionSyntax member)
            root = member.Expression;
        while (root is ElementAccessExpressionSyntax element)
            root = element.Expression;

        if (context.SemanticModel.GetTypeInfo(root, context.CancellationToken).Type
                is INamedTypeSymbol entity
            && touched.Contains(entity))
            written.Add(entity);
    }

    // The token the doctor can see: a member marked [Timestamp]/[ConcurrencyCheck], or a property named
    // RowVersion (the conventional name that makes fluent-only configuration visible to static analysis).
    private static bool HasConcurrencyToken(INamedTypeSymbol entity) =>
        entity.GetMembers().Any(member =>
            member.Name == "RowVersion"
            || member.GetAttributes().Any(a => a.AttributeClass?.Name
                    is "Timestamp" or "TimestampAttribute"
                    or "ConcurrencyCheck" or "ConcurrencyCheckAttribute"));

}
