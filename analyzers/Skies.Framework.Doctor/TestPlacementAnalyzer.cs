using System;
using System.Collections.Immutable;
using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0029 — tests live in a spec. Every test in a Skies app sits in a <c>.specs/&lt;id&gt;-&lt;slug&gt;/e2e/</c>
/// folder, titled after the failure mode it covers, because a spec is the only place a test is tied to a written
/// failure mode and to a receipt that saw it fail first. A test anywhere else was written as coverage: nothing says
/// what it guards, nothing proved it can fail, and it rots into a suite nobody trusts. An isolated system (a value
/// object, a calculation, a parser) is no exception; it gets its own spec whose <c>e2e/</c> holds isolated cases.
///
/// The rule is about where tests live, not whether they exist: it never asks for a test. It looks at the path of the
/// file a test method is declared in and flags the method when no directory segment is <c>.specs</c>. A test method is
/// one carrying xUnit <c>[Fact]</c>/<c>[Theory]</c>, NUnit <c>[Test]</c>/<c>[TestCase]</c>/<c>[TestCaseSource]</c>/
/// <c>[Theory]</c>, or MSTest <c>[TestMethod]</c>/<c>[DataTestMethod]</c>, including attributes derived from them
/// (<c>[SkippableFact]</c>). When the test framework does not resolve (a stray test compiled into the API project,
/// which does not reference xUnit), the attribute's name decides, so the stray file is still named for what it is.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class TestPlacementAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for a test method declared outside a spec folder.</summary>
    public const string DiagnosticId = "SKY0029";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "Tests live in a spec",
        messageFormat: "tests live in a spec: move '{0}' into .specs/<id>-<slug>/e2e/ and name it after the failure "
                     + "mode it covers",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "A test outside .specs/ is coverage without a failure mode: nothing states what it guards and no "
                   + "receipt saw it fail. Move it into a spec's e2e/ folder and title it FM-n.");

    // The attribute types that make a method a test, by full metadata name. Derived attributes match through their
    // base types (xUnit's Theory derives from Fact, MSTest's DataTestMethod from TestMethod).
    private static readonly ImmutableHashSet<string> TestAttributes = ImmutableHashSet.Create(
        StringComparer.Ordinal,
        "Xunit.FactAttribute",
        "Xunit.TheoryAttribute",
        "NUnit.Framework.TestAttribute",
        "NUnit.Framework.TestCaseAttribute",
        "NUnit.Framework.TestCaseSourceAttribute",
        "NUnit.Framework.TheoryAttribute",
        "Microsoft.VisualStudio.TestTools.UnitTesting.TestMethodAttribute",
        "Microsoft.VisualStudio.TestTools.UnitTesting.DataTestMethodAttribute");

    // The same attributes by the name written in source, for when the framework does not resolve.
    private static readonly ImmutableHashSet<string> TestAttributeNames = ImmutableHashSet.Create(
        StringComparer.Ordinal,
        "Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestMethod", "DataTestMethod");

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterSyntaxNodeAction(Analyze, SyntaxKind.MethodDeclaration);
    }

    private static void Analyze(SyntaxNodeAnalysisContext context)
    {
        var method = (MethodDeclarationSyntax)context.Node;
        if (method.AttributeLists.Count == 0 || IsInSpec(method.SyntaxTree.FilePath))
            return;

        var isTest = method.AttributeLists
            .SelectMany(list => list.Attributes)
            .Any(attribute => IsTestAttribute(attribute, context.SemanticModel, context));
        if (!isTest)
            return;

        var owner = method.Parent is TypeDeclarationSyntax type ? type.Identifier.Text + "." : "";
        context.ReportDiagnostic(Diagnostic.Create(Rule, method.Identifier.GetLocation(), owner + method.Identifier.Text));
    }

    // A spec's cases are compiled from `.specs/<id>-<slug>/e2e/`; any `.specs` directory segment counts, so the rule
    // holds whether the tests project globs the folder in or a tool copies it elsewhere under a spec tree.
    private static bool IsInSpec(string? path) =>
        path is not null && ("/" + path.Replace('\\', '/')).Contains("/.specs/");

    private static bool IsTestAttribute(AttributeSyntax attribute, SemanticModel model, SyntaxNodeAnalysisContext context)
    {
        var type = (model.GetSymbolInfo(attribute, context.CancellationToken).Symbol as IMethodSymbol)?.ContainingType;
        if (type is not null && type.TypeKind != TypeKind.Error)
        {
            for (var current = type; current is not null; current = current.BaseType)
                if (TestAttributes.Contains(FullName(current)))
                    return true;
            return false;
        }
        return IsTestAttributeName(WrittenName(attribute.Name));
    }

    // `Xunit.Fact`, `global::Xunit.FactAttribute` and `Fact` all name the same attribute; compare the last segment
    // without its `Attribute` suffix. A custom `*Fact`/`*Theory` (SkippableFact) is a test by its name too.
    private static bool IsTestAttributeName(string name)
    {
        var bare = name.EndsWith("Attribute", StringComparison.Ordinal) ? name.Substring(0, name.Length - 9) : name;
        return TestAttributeNames.Contains(bare)
               || bare.EndsWith("Fact", StringComparison.Ordinal)
               || bare.EndsWith("Theory", StringComparison.Ordinal);
    }

    private static string WrittenName(NameSyntax name) => name switch
    {
        QualifiedNameSyntax qualified => qualified.Right.Identifier.Text,
        AliasQualifiedNameSyntax alias => alias.Name.Identifier.Text,
        SimpleNameSyntax simple => simple.Identifier.Text,
        _ => name.ToString(),
    };

    private static string FullName(INamedTypeSymbol type)
    {
        var ns = type.ContainingNamespace;
        return ns is null || ns.IsGlobalNamespace ? type.MetadataName : ns.ToDisplayString() + "." + type.MetadataName;
    }
}
