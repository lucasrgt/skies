namespace Skies.Framework.Abstractions;

/// <summary>
/// Marks a domain entity whose factories and operations own its state transitions.
/// The doctor checks encapsulation: no accessible constructor or property setter/init accessor.
/// Validation behavior belongs to the entity's implementation and tests; no helper name proves it.
/// This marker has no EF or runtime semantics. Removing the analyzer leaves the code unchanged.
/// </summary>
[AttributeUsage(AttributeTargets.Class, AllowMultiple = false, Inherited = false)]
public sealed class EntityAttribute : Attribute;
