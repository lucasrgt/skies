using System.Linq;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace Skies.Framework.Doctor;

/// <summary>
/// The "what module does this slice belong to?" reading — the convention that a slice's module is the
/// last segment of its enclosing namespace (<c>App.Api.Modules.Wallets</c> → <c>Wallets</c>; the
/// <c>Slices/</c> subfolder is not part of the namespace).
/// </summary>
internal static class ModuleNaming
{
    /// <summary>
    /// The module a slice class belongs to: the last segment of its enclosing namespace, or <see langword="null"/>
    /// when the class sits in no namespace (the module convention does not apply).
    /// </summary>
    public static string? ModuleOf(ClassDeclarationSyntax cls)
    {
        var name = cls.Ancestors().OfType<BaseNamespaceDeclarationSyntax>().FirstOrDefault()?.Name.ToString();
        if (string.IsNullOrEmpty(name))
            return null;
        var dot = name!.LastIndexOf('.');
        return dot >= 0 ? name.Substring(dot + 1) : name;
    }
}
