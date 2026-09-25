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
    /// Shared template folders under `templates/dotnet` the flow also emits (skipped when already present), so two
    /// flows needing the same entity and store write it once.
    pub shared_folders: &'static [&'static str],
    /// Domain registrations added to `AccountModule.AddServices`. External providers belong to Platform.
    pub di_lines: &'static [&'static str],
    /// Base registrations the flow swaps for its own (old line, new line), so a service keeps one registration.
    pub di_replacements: &'static [(&'static str, &'static str)],
    /// The local provider registration, added only to the development platform.
    pub provider_line: &'static str,
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

/// The framework's verification service over the app's store, shared by the phone and email flows.
const VERIFICATION_DI: &str = "services.AddVerificationTokens<VerificationTokenStore>();";

/// The one table behind every verification secret (phone codes and email links), shared by the phone and email flows.
const VERIFICATION_DB_SETS: &[(&str, &str)] = &[(
    "VerificationToken",
    "    public DbSet<VerificationToken> VerificationTokens => Set<VerificationToken>();",
)];

/// Link tokens are found by hash; codes by their user and purpose.
const VERIFICATION_INDEXES: &[&str] = &[
    "        model.Entity<VerificationToken>().HasIndex(t => t.SecretHash);",
    "        model.Entity<VerificationToken>().HasIndex(t => new { t.UserId, t.Purpose });",
];

static OTP: FlowSpec = FlowSpec {
    token: "otp",
    folder: "auth-otp",
    package_id: "Skies.Framework.Sms",
    provider_namespace: "Skies.Framework.Sms",
    shared_folders: &["auth-verification"],
    di_lines: &[VERIFICATION_DI],
    di_replacements: &[],
    provider_line: "services.AddSingleton<ISmsSender, ConsoleSmsSender>();",
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
    db_sets: VERIFICATION_DB_SETS,
    indexes: VERIFICATION_INDEXES,
    map_lines: &[
        "        ResendPhoneCode.Map(credentials);",
        "        VerifyPhone.Map(credentials);",
    ],
    error_codes: &[
        (
            "NoActiveCode",
            "auth.no_active_code",
            "The phone has no active OTP code.",
        ),
        (
            "InvalidCode",
            "auth.invalid_code",
            "The submitted OTP code is wrong or missing.",
        ),
        (
            "TooManyAttempts",
            "auth.too_many_attempts",
            "The OTP code was guessed wrong too many times and is now locked.",
        ),
        (
            "PhoneRequired",
            "account.phone_required",
            "No phone number was given to send the code to.",
        ),
    ],
    summary: "auth:otp generated — phone verification by SMS code (ConsoleSmsSender in dev).",
};

static OAUTH: FlowSpec = FlowSpec {
    token: "oauth",
    folder: "auth-oauth",
    package_id: "Skies.Framework.Identity",
    provider_namespace: "Skies.Framework.Identity",
    shared_folders: &[],
    di_lines: &[],
    di_replacements: &[],
    provider_line: "services.AddSingleton<IExternalIdentityVerifier, FakeExternalIdentity>();",
    user_fields: &[
        "    /// <summary>Whether the account's email has been verified.</summary>\n    public bool IsEmailVerified { get; private set; }",
    ],
    user_methods: &[UserMethod {
        token: "RegisterViaGoogle(",
        code: "    /// <summary>Register an account in <paramref name=\"orgId\"/> from a Google identity: Google has\n    \
               /// already verified the email, so the user is email-verified from the start, has no usable password\n    \
               /// (Google is the credential), and lands at PhonePending. Funnels through EnsureValid.</summary>\n    \
               public static Result<User> RegisterViaGoogle(Guid orgId, Email email, DateTime now) =>\n        new User\n        \
               {\n            Id = Guid.NewGuid(),\n            OrgId = orgId,\n            Email = email,\n            Name = email.Value,\n            \
               PasswordHash = PasswordHash.None,\n            IsEmailVerified = true,\n            \
               RegistrationStep = RegistrationStep.PhonePending,\n            CreatedAt = now,\n        }.EnsureValid();",
    }],
    db_sets: &[],
    indexes: &[],
    map_lines: &[
        "        RegisterWithGoogle.Map(credentials);",
        "        LoginWithGoogle.Map(credentials);",
    ],
    error_codes: &[
        (
            "InvalidToken",
            "auth.invalid_token",
            "The external identity token is invalid.",
        ),
        (
            "EmailTaken",
            "account.email_taken",
            "An account already exists for the external identity's email.",
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
    shared_folders: &["auth-verification"],
    di_lines: &[VERIFICATION_DI],
    di_replacements: &[(
        "services.AddSingleton<IAccountNotices, NoAccountNotices>();",
        "services.AddSingleton<IAccountNotices, EmailAccountNotices>();",
    )],
    provider_line: "services.AddSingleton<IEmailSender, ConsoleEmailSender>();",
    user_fields: &[
        "    /// <summary>Whether the account's email has been verified.</summary>\n    public bool IsEmailVerified { get; private set; }",
    ],
    user_methods: &[UserMethod {
        token: "MarkEmailVerified(",
        code: "    /// <summary>Flag the account's email verified. Cannot fail — a void mutation.</summary>\n    \
               public void MarkEmailVerified() => IsEmailVerified = true;",
    }],
    db_sets: VERIFICATION_DB_SETS,
    indexes: VERIFICATION_INDEXES,
    map_lines: &[
        "        RequestEmailVerification.Map(credentials);",
        "        VerifyEmail.Map(credentials);",
        "        RequestPasswordReset.Map(credentials);",
        "        ResetPassword.Map(credentials);",
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
