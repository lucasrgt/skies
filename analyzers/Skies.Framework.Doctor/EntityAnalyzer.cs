using System.Collections.Immutable;
using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// Enforces entity encapsulation: construction and state changes stay behind the type's methods.
/// Validation belongs to the factory and domain operations; this rule does not prove their behavior
/// or require a particular validation-helper name.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class EntityAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for an entity that does not encapsulate its state or its invariants.</summary>
    public const string DiagnosticId = "SKY0014";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "Entity must encapsulate its state",
        messageFormat: "Entity '{0}' {1}",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "An [Entity] exposes no public constructor, setter, or init accessor. "
                   + "Its factories and domain operations own validation.");

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
        if (!IsEntity(cls))
            return;

        var name = cls.Identifier.Text;
        var at = cls.Identifier.GetLocation();

        if (cls.ParameterList is not null)
            Report(context, at, name, "must not expose a primary constructor — open it through a static factory "
                                    + "(e.g. Open) and keep a private parameterless constructor for EF");

        var constructors = cls.Members.OfType<ConstructorDeclarationSyntax>()
            .Where(c => !c.Modifiers.Any(SyntaxKind.StaticKeyword)).ToList();
        foreach (var ctor in constructors.Where(c => IsAccessible(c.Modifiers)))
            Report(context, ctor.Identifier.GetLocation(), name,
                "must not expose a public constructor — open it through a static factory (e.g. Open)");

        // With no declared constructor the compiler emits a public parameterless one — a public way in. The
        // entity must declare a private one (the same one EF materialises through).
        if (constructors.Count == 0)
            Report(context, at, name, "must declare a private parameterless constructor for EF materialisation "
                                    + "(and a static factory to open it), so there is no public way to construct it");

        foreach (var prop in cls.Members.OfType<PropertyDeclarationSyntax>().Where(HasPublicSetter))
            Report(context, prop.Identifier.GetLocation(), name,
                "must encapsulate its state — property '" + prop.Identifier.Text + "' has a public setter or init accessor; "
                + "change it through a method and declare it '{ get; private set; }'");
    }

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

    private static bool IsEntity(ClassDeclarationSyntax cls) =>
        cls.AttributeLists.SelectMany(list => list.Attributes).Select(attr => attr.Name.ToString())
            .Any(n => n is "Entity" or "EntityAttribute"
                   || n.EndsWith(".Entity") || n.EndsWith(".EntityAttribute"));

    private static void Report(SyntaxNodeAnalysisContext context, Location location, string name, string problem) =>
        context.ReportDiagnostic(Diagnostic.Create(Rule, location, name, problem));
}
