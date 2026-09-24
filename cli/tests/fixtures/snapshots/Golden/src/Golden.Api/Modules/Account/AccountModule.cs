namespace Golden.Api.Modules.Account;

/// <summary>The Account module groups its slices under /account.</summary>
public static class AccountModule
{
    public static void Map(IEndpointRouteBuilder app)
    {
        var account = app.MapGroup("/account");
        Register.Map(account);
        Login.Map(account);
        Refresh.Map(account);
        Logout.Map(account);
        Me.Map(account);
        ListMySessions.Map(account);
        RevokeSession.Map(account);
        RevokeOtherSessions.Map(account);
        ResendPhoneCode.Map(account);
        VerifyPhone.Map(account);
        RegisterWithGoogle.Map(account);
        LoginWithGoogle.Map(account);
        RequestEmailVerification.Map(account);
        VerifyEmail.Map(account);
        RequestPasswordReset.Map(account);
        ResetPassword.Map(account);
    }
}
