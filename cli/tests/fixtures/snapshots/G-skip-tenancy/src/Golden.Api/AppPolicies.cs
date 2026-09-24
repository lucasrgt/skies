namespace Golden.Api;

/// <summary>The app's named authorization policies: the ones a route group anywhere in the app requires by name, so
/// the name is a constant rather than a string repeated per module. The Account module registers each of them.</summary>
public static class AppPolicies
{
    /// <summary>May change app-wide data: the rows every user shares. Held by a user with the <c>Admin</c> role, which
    /// registration never grants; the operator assigns it.</summary>
    public const string AppAdmin = "app-admin";
}
