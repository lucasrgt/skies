namespace Skies.Framework.Abstractions;

/// <summary>
/// Marks an immutable domain value constructed through a factory returning <see cref="Result{T}"/>.
/// The doctor checks constructors, property accessors, and the factory signature; the factory owns validation.
/// Structs still admit <c>default(T)</c>, so use a class when the zero state cannot represent a valid value.
/// This marker has no runtime behavior. Removing the analyzer leaves the code unchanged.
/// </summary>
[AttributeUsage(AttributeTargets.Struct | AttributeTargets.Class, AllowMultiple = false, Inherited = false)]
public sealed class ValueObjectAttribute : Attribute;
