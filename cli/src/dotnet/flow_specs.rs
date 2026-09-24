//! What each auth sub-flow adds to an Account module: the data behind `g auth:otp|oauth|email`.
//!
//! Kept apart from the editing logic in `flows.rs` so a reviewer can read one flow's footprint (fields, DbSets,
//! routes, error codes, provider) in one place.

/// The provider-backed flows a generated Account module can be augmented with.
#[derive(Clone, Copy, Debug)]
pub enum Flow {
    /// Phone verification by one-time SMS code (`Skies.Framework.Sms`).
    Otp,
    /// Sign-up and sign-in with an external OIDC identity such as Google (`Skies.Framework.Identity`).
    OAuth,
    /// Email verification and password reset by emailed token (`Skies.Framework.Mail`).
    Email,
}

/// A method added to `User`, detected by `token` so re-running (or a second flow sharing it) never duplicates it.
pub struct UserMethod {
    pub token: &'static str,
    pub code: &'static str,
}

pub struct FlowSpec {
    /// The command suffix: `g auth:<token>`.
    pub token: &'static str,
    /// The template folder under `templates/dotnet` and the spec template under `templates/dotnet/specs`.
    pub folder: &'static str,
    pub package_id: &'static str,
    pub provider_namespace: &'static str,
    /// The dev provider registration, added to `AccountSetup.AddAccount` (never `Program.cs`, SKY0017).
    pub di_line: &'static str,
    pub user_fields: &'static [&'static str],
    pub user_methods: &'static [UserMethod],
    /// (entity type, DbSet declaration)
    pub db_sets: &'static [(&'static str, &'static str)],
    pub indexes: &'static [&'static str],
    pub map_lines: &'static [&'static str],
    /// (constant, value, summary)
    pub error_codes: &'static [(&'static str, &'static str, &'static str)],
    pub summary: &'static str,
}

impl Flow {
    pub fn spec(self) -> &'static FlowSpec {
        match self {
            Flow::Otp => &OTP,
            Flow::OAuth => &OAUTH,
            Flow::Email => &EMAIL,
        }
    }
}

static OTP: FlowSpec = FlowSpec {
    token: "otp",
    folder: "auth-otp",
    package_id: "Skies.Framework.Sms",
    provider_namespace: "Skies.Framework.Sms",
    di_line: "builder.Services.AddSingleton<ISmsSender, ConsoleSmsSender>();",
    user_fields: &[
        "    /// <summary>The verified phone number, set once VerifyPhone succeeds.</summary>\n    public string? Phone { get; private set; }",
        "    /// <summary>Whether the phone number has been verified.</summary>\n    public bool IsPhoneVerified { get; private set; }",
    ],
    user_methods: &[UserMethod {
        token: "CompletePhoneVerification(",
        code: "    /// <summary>Complete phone verification: record the verified phone, flag it verified, and\n    \
               /// advance registration to Complete. Cannot fail — a void mutation.</summary>\n    \
               public void CompletePhoneVerification(string phone)\n    {\n        Phone = phone;\n        \
               IsPhoneVerified = true;\n        RegistrationStep = RegistrationStep.Complete;\n    }",
    }],
    db_sets: &[("PhoneOtp", "    public DbSet<PhoneOtp> PhoneOtps => Set<PhoneOtp>();")],
    indexes: &["        model.Entity<PhoneOtp>().HasIndex(o => o.UserId);"],
    map_lines: &[
        "        ResendPhoneCode.Map(account);",
        "        VerifyPhone.Map(account);",
    ],
    error_codes: &[
        (
            "NoActiveCode",
            "auth.no_active_code",
            "The phone has no active OTP code.",
        ),
        ("InvalidCode", "auth.invalid_code", "The submitted OTP code is wrong."),
        (
            "TooManyAttempts",
            "auth.too_many_attempts",
            "The OTP code was guessed wrong too many times and is now locked.",
        ),
    ],
    summary: "auth:otp generated — phone verification by SMS code (ConsoleSmsSender in dev).",
};

static OAUTH: FlowSpec = FlowSpec {
    token: "oauth",
    folder: "auth-oauth",
    package_id: "Skies.Framework.Identity",
    provider_namespace: "Skies.Framework.Identity",
    di_line: "builder.Services.AddSingleton<IExternalIdentity, FakeExternalIdentity>();",
    user_fields: &[
        "    /// <summary>Whether the account's email has been verified.</summary>\n    public bool IsEmailVerified { get; private set; }",
    ],
    user_methods: &[UserMethod {
        token: "RegisterViaGoogle(",
        code: "    /// <summary>Register an account from a Google identity: Google has already verified the email,\n    \
               /// so the user is email-verified from the start, has no password (a random one is stored —\n    \
               /// Google is the credential), and lands at PhonePending. Funnels through EnsureValid.</summary>\n    \
               public static Result<User> RegisterViaGoogle(Email email, DateTime now) =>\n        new User\n        \
               {\n            Id = Guid.NewGuid(),\n            Email = email,\n            Name = email.Value,\n            \
               PasswordHash = PasswordHash.Create(Guid.NewGuid().ToString()),\n            IsEmailVerified = true,\n            \
               RegistrationStep = RegistrationStep.PhonePending,\n            CreatedAt = now,\n        }.EnsureValid();",
    }],
    db_sets: &[],
    indexes: &[],
    map_lines: &[
        "        RegisterWithGoogle.Map(account);",
        "        LoginWithGoogle.Map(account);",
    ],
    error_codes: &[
        (
            "InvalidToken",
            "auth.invalid_token",
            "The external identity token is invalid.",
        ),
        (
            "NoAccount",
            "auth.no_account",
            "No account exists for this external identity.",
        ),
    ],
    summary: "auth:oauth generated — Google sign-up/sign-in (FakeExternalIdentity in dev).",
};

static EMAIL: FlowSpec = FlowSpec {
    token: "email",
    folder: "auth-email",
    package_id: "Skies.Framework.Mail",
    provider_namespace: "Skies.Framework.Mail",
    di_line: "builder.Services.AddSingleton<IEmailSender, ConsoleEmailSender>();",
    user_fields: &[
        "    /// <summary>Whether the account's email has been verified.</summary>\n    public bool IsEmailVerified { get; private set; }",
    ],
    user_methods: &[UserMethod {
        token: "MarkEmailVerified(",
        code: "    /// <summary>Flag the account's email verified. Cannot fail — a void mutation.</summary>\n    \
               public void MarkEmailVerified() => IsEmailVerified = true;",
    }],
    db_sets: &[
        (
            "EmailVerificationToken",
            "    public DbSet<EmailVerificationToken> EmailVerificationTokens => Set<EmailVerificationToken>();",
        ),
        (
            "PasswordResetToken",
            "    public DbSet<PasswordResetToken> PasswordResetTokens => Set<PasswordResetToken>();",
        ),
    ],
    indexes: &[
        "        model.Entity<EmailVerificationToken>().HasIndex(t => t.TokenHash);",
        "        model.Entity<PasswordResetToken>().HasIndex(t => t.TokenHash);",
    ],
    map_lines: &[
        "        RequestEmailVerification.Map(account);",
        "        VerifyEmail.Map(account);",
        "        RequestPasswordReset.Map(account);",
        "        ResetPassword.Map(account);",
    ],
    error_codes: &[
        (
            "InvalidToken",
            "auth.invalid_token",
            "The verification token is invalid or expired.",
        ),
        (
            "ResetTokenInvalid",
            "auth.invalid_reset_token",
            "The password-reset token is invalid or expired.",
        ),
    ],
    summary: "auth:email generated — email verification + password reset by emailed token (ConsoleEmailSender in dev).",
};
