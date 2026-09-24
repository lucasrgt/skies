namespace Golden.Api.Modules.Account;

/// <summary>What a user is. Admin is the app's operator: it satisfies <see cref="AppPolicies.AppAdmin"/>, the
/// policy on writes to app-wide data, so registration never grants it and the operator assigns it out of band. Extend
/// this enum as the app's authorization model grows; a role that runs one org's settings is a new value, not Admin.</summary>
public enum Role
{
    Member,
    Admin,
}
