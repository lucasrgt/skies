using System.Collections.Immutable;
using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// Keeps value objects immutable and routes explicit construction through a Result-returning factory.
/// Factories own validation. Structs still admit default(T); choose a class when the zero state is invalid.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class ValueObjectAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for a value object that is not encapsulated.</summary>
    public const string DiagnosticId = "SKY0013";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "Value object must encapsulate construction and state",
        messageFormat: "Value object '{0}' {1}",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "A [ValueObject] is immutable and built only through a static smart constructor "
                   + "returning Result<T> — no public constructor, no public setter (the Money.From shape).");

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterSyntaxNodeAction(Analyze,
            SyntaxKind.ClassDeclaration, SyntaxKind.StructDeclaration,
            SyntaxKind.RecordDeclaration, SyntaxKind.RecordStructDeclaration);
    }

    private static void Analyze(SyntaxNodeAnalysisContext context)
    {
        var type = (TypeDeclarationSyntax)context.Node;
        if (!IsValueObject(type))
            return;

        var name = type.Identifier.Text;
        var at = type.Identifier.GetLocation();

        // A primary/positional constructor (record positional params or a C# 12 primary ctor) is a public
        // way in that skips the smart constructor — so construction no longer goes through that factory.
        if (type.ParameterList is not null)
            Report(context, at, name, "must not expose a primary/positional constructor — build it through a "
                                    + "static smart constructor returning Result<" + name + "> (e.g. From)");

        foreach (var ctor in type.Members.OfType<ConstructorDeclarationSyntax>().Where(c => IsAccessible(c.Modifiers)))
            Report(context, ctor.Identifier.GetLocation(), name,
                "must not expose a public constructor — build it through a static smart constructor "
                + "returning Result<" + name + "> (e.g. From)");

        if (type is ClassDeclarationSyntax || type.IsKind(SyntaxKind.RecordDeclaration))
        {
            if (type.ParameterList is null && !type.Members.OfType<ConstructorDeclarationSyntax>().Any(c => !c.Modifiers.Any(SyntaxKind.StaticKeyword)))
                Report(context, at, name, "must declare a private constructor; the implicit constructor is public");
        }

        if (!HasSmartConstructor(type, name))
            Report(context, at, name, "must declare a public static smart constructor returning Result<" + name
                                    + "> (e.g. From) — the only way to build a valid instance");

        foreach (var prop in type.Members.OfType<PropertyDeclarationSyntax>().Where(HasPublicSetter))
            Report(context, prop.Identifier.GetLocation(), name,
                "must be immutable — property '" + prop.Identifier.Text + "' has a public setter or init accessor; use "
                + "'{ get; }' or a private init accessor");
    }

    // A public/internal static method whose return type is Result<Name> — the smart constructor.
    private static bool HasSmartConstructor(TypeDeclarationSyntax type, string name) =>
        type.Members.OfType<MethodDeclarationSyntax>()
            .Any(m => m.Modifiers.Any(SyntaxKind.StaticKeyword)
                   && IsAccessible(m.Modifiers)
                   && m.ReturnType.ToString().Replace(" ", "").Contains("Result<" + name + ">"));

    private static bool HasPublicSetter(PropertyDeclarationSyntax prop)
    {
        if (!IsAccessible(prop.Modifiers))
            return false;
        var setter = prop.AccessorList?.Accessors.FirstOrDefault(a => a.IsKind(SyntaxKind.SetAccessorDeclaration) || a.IsKind(SyntaxKind.InitAccessorDeclaration));
        return setter is not null && !RestrictsAccess(setter.Modifiers);
    }

    private static bool IsAccessible(SyntaxTokenList modifiers) =>
        modifiers.Any(SyntaxKind.PublicKeyword) || modifiers.Any(SyntaxKind.InternalKeyword)
        || modifiers.Any(SyntaxKind.ProtectedKeyword);

    private static bool RestrictsAccess(SyntaxTokenList modifiers) =>
        modifiers.Any(SyntaxKind.PrivateKeyword) && !modifiers.Any(SyntaxKind.ProtectedKeyword);

    private static bool IsValueObject(TypeDeclarationSyntax type) =>
        type.AttributeLists.SelectMany(list => list.Attributes).Select(attr => attr.Name.ToString())
            .Any(n => n is "ValueObject" or "ValueObjectAttribute"
                   || n.EndsWith(".ValueObject") || n.EndsWith(".ValueObjectAttribute"));

    private static void Report(SyntaxNodeAnalysisContext context, Location location, string name, string problem) =>
        context.ReportDiagnostic(Diagnostic.Create(Rule, location, name, problem));
}
