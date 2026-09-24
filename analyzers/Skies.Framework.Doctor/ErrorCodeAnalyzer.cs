using System;
using System.Collections.Concurrent;
using System.Collections.Immutable;
using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.Diagnostics;
using Microsoft.CodeAnalysis.Operations;

namespace Skies.Framework.Doctor;

/// <summary>
/// <c>SKY0018</c> — an error code is a registry constant, not an inline literal. Every <c>code</c> passed to an
/// <c>Error</c> factory, to <c>Validation.Check</c> / <c>Validation.Add</c>, or to a <c>FieldError</c>
/// must reference a <c>const</c> on a class whose name ends with <c>ErrorCodes</c> (e.g.
/// <c>WalletsErrorCodes.NotFound</c>) — never a bare string.
///
/// The point is discoverability of the whole set: codes named in one registry per module can be enumerated (by
/// reflection at the boundary) into the OpenAPI document as the <c>ErrorBody.Code</c> enum, so the generated
/// client is typed on them and the frontend's i18n can be checked exhaustively. An inline literal is invisible
/// to that — it would reach a user untranslated. The rule keys off the parameter <em>named</em> <c>code</c>, so
/// it covers every shape uniformly and leaves codeless calls (<c>Collect</c>, the aggregate
/// <c>Error.Validation(fields)</c>) alone.
///
/// <c>SKY0019</c> is the reverse direction — every constant on an <c>*ErrorCodes</c> registry must be referenced
/// somewhere in the compilation. A code left unused (a flow was dropped but its constant lingers) is silent
/// drift: it still ships in the OpenAPI <c>ErrorBody</c> enum and the frontend i18n catalog. Together the two
/// rules make the registry the exact, live set of codes — no orphans, no inline literals. <c>SKY0019</c> is a
/// warning: a dead code is untidy contract, not a broken one, and it is often the pending half of a flow being
/// written.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class ErrorCodeAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for an error code that is not a registry constant.</summary>
    public const string DiagnosticId = "SKY0018";

    /// <summary>The identifier reported for an <c>*ErrorCodes</c> constant that is declared but never used.</summary>
    public const string DeadCodeDiagnosticId = "SKY0019";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "Error code must be a registry constant",
        messageFormat: "An error code must reference a constant on an *ErrorCodes registry, not {0}",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "Every error code (the 'code' argument of an Error factory / Validation.Check / Validation.Add / "
                   + "FieldError) must be a const declared on a class named *ErrorCodes, never an inline literal — so "
                   + "the full set of codes stays discoverable: the OpenAPI document enumerates it for the typed "
                   + "client and the frontend's i18n is checked exhaustively against it.");

    private static readonly DiagnosticDescriptor DeadCodeRule = new(
        id: DeadCodeDiagnosticId,
        title: "Error code constant must be used",
        messageFormat: "The error code '{0}' on {1} is declared but never used — remove it or wire the path that raises it",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Warning,
        isEnabledByDefault: true,
        description: "Every constant on an *ErrorCodes registry must be referenced by an Error factory / Validation "
                   + "call somewhere in the compilation. A dead code is silent drift — it still ships in the OpenAPI "
                   + "ErrorBody enum and the frontend i18n catalog — so dropping a flow must remove its codes too.",
        customTags: WellKnownDiagnosticTags.CompilationEnd);

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics =>
        ImmutableArray.Create(Rule, DeadCodeRule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterOperationAction(c => CheckCode(c, ((IInvocationOperation)c.Operation).TargetMethod.ContainingType,
            ((IInvocationOperation)c.Operation).Arguments), OperationKind.Invocation);
        context.RegisterOperationAction(c => CheckCode(c, ((IObjectCreationOperation)c.Operation).Constructor?.ContainingType,
            ((IObjectCreationOperation)c.Operation).Arguments), OperationKind.ObjectCreation);
        context.RegisterCompilationStartAction(AnalyzeDeadCodes);
    }

    // SKY0019 — every *ErrorCodes constant must be referenced somewhere in the compilation. Collect the declared
    // constants and the referenced ones, then at compilation end flag any declared-but-never-referenced (a code
    // whose path was removed but whose constant — and so its OpenAPI enum entry + i18n key — still lingers).
    private static void AnalyzeDeadCodes(CompilationStartAnalysisContext context)
    {
        var declared = new ConcurrentDictionary<IFieldSymbol, Location>(SymbolEqualityComparer.Default);
        var referenced = new ConcurrentDictionary<IFieldSymbol, byte>(SymbolEqualityComparer.Default);

        context.RegisterSymbolAction(symbolContext =>
        {
            var field = (IFieldSymbol)symbolContext.Symbol;
            if (IsCodeConstant(field) && field.Locations.FirstOrDefault() is { } location)
                declared.TryAdd(field, location);
        }, SymbolKind.Field);

        context.RegisterOperationAction(operationContext =>
        {
            var field = ((IFieldReferenceOperation)operationContext.Operation).Field;
            if (IsCodeConstant(field))
                referenced.TryAdd(field, 0);
        }, OperationKind.FieldReference);

        context.RegisterCompilationEndAction(endContext =>
        {
            foreach (var entry in declared)
                if (!referenced.ContainsKey(entry.Key))
                    endContext.ReportDiagnostic(Diagnostic.Create(
                        DeadCodeRule, entry.Value, entry.Key.Name, entry.Key.ContainingType.Name));
        });
    }

    // A code constant: a public string const on a class whose name ends with ErrorCodes.
    private static bool IsCodeConstant(IFieldSymbol field) =>
        field is { IsConst: true, DeclaredAccessibility: Accessibility.Public, Type.SpecialType: SpecialType.System_String }
        && field.ContainingType.Name.EndsWith("ErrorCodes", StringComparison.Ordinal);

    // Only the framework's error-carrying shapes declare a 'code' parameter we care about.
    private static bool IsErrorShape(INamedTypeSymbol? type) =>
        type?.Name is "Error" or "Validation" or "FieldError";

    private static void CheckCode(OperationAnalysisContext context, INamedTypeSymbol? containingType,
        ImmutableArray<IArgumentOperation> arguments)
    {
        if (!IsErrorShape(containingType))
            return;

        foreach (var argument in arguments)
        {
            if (argument.Parameter is not { } parameter
                || !string.Equals(parameter.Name, "code", StringComparison.OrdinalIgnoreCase))
                continue;

            var value = argument.Value is IConversionOperation conversion ? conversion.Operand : argument.Value;
            if (value is IFieldReferenceOperation { Field: { IsConst: true } field }
                && field.ContainingType.Name.EndsWith("ErrorCodes", StringComparison.Ordinal))
                return; // a registry constant — conformant

            var what = value is ILiteralOperation ? "an inline literal" : "a non-registry value";
            context.ReportDiagnostic(Diagnostic.Create(Rule, value.Syntax.GetLocation(), what));
            return;
        }
    }
}
