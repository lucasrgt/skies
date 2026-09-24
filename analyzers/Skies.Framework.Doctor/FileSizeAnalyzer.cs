using System.Collections.Immutable;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.Diagnostics;
using Microsoft.CodeAnalysis.Text;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0007 — keeps every source file in a Skies app at or under 500 lines, the Rails-repo discipline
/// the framework holds itself to (the user-app counterpart of <c>SKYSELF001</c>, which polices the
/// framework's own libraries). A file past the ceiling is the signal to extract a concern into its own
/// file — a vertical slice should be small — not to keep packing. EF migrations (a <c>Migrations</c>
/// directory) are exempt: tool-emitted, append-only, never hand-maintained — their size is the schema's,
/// not a packing smell (the hostpoint pilot's InitialCreate crossed the ceiling on real tables alone).
/// A warning, not an error: the ceiling is a readability taste, not an architectural boundary, so a long file
/// is a prompt to split, never a broken build.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class FileSizeAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The maximum number of lines a single source file may contain.</summary>
    public const int MaxLines = 500;

    /// <summary>The identifier reported when a file exceeds <see cref="MaxLines"/>.</summary>
    public const string DiagnosticId = "SKY0007";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "Source file exceeds the line ceiling",
        messageFormat: "File has {0} lines; the ceiling is {1}. Extract a concern into its own file.",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Warning,
        isEnabledByDefault: true,
        description: "Skies app files stay at or under the ceiling so each holds a single, readable concern.");

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterSyntaxTreeAction(AnalyzeTree);
    }

    private static void AnalyzeTree(SyntaxTreeAnalysisContext context)
    {
        if (IsMigration(context.Tree.FilePath))
            return;   // tool-emitted, append-only — its size is the schema's, not a packing smell

        var lineCount = context.Tree.GetText(context.CancellationToken).Lines.Count;
        if (lineCount <= MaxLines)
            return;

        var location = Location.Create(context.Tree, new TextSpan(0, 0));
        context.ReportDiagnostic(Diagnostic.Create(Rule, location, lineCount, MaxLines));
    }

    private static bool IsMigration(string? path) =>
        path is not null
        && path.Replace('\\', '/').Contains("/Migrations/");
}
