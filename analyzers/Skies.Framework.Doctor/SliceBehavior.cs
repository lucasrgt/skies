using System.Linq;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace Skies.Framework.Doctor;

/// <summary>
/// Recognizes a slice by its <c>[Slice]</c> marker textually, so a rule needs no reference to
/// Skies.Framework.Abstractions to know which classes the slice conventions govern.
/// </summary>
internal static class SliceBehavior
{
    /// <summary>Whether a class carries the <c>[Slice]</c> marker.</summary>
    public static bool IsSlice(ClassDeclarationSyntax cls) =>
        cls.AttributeLists
            .SelectMany(list => list.Attributes)
            .Select(attribute => attribute.Name.ToString())
            .Any(candidate => candidate is "Slice" or "SliceAttribute"
                || candidate.EndsWith(".Slice") || candidate.EndsWith(".SliceAttribute"));
}
