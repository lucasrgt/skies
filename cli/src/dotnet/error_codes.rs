//! The `<Module>ErrorCodes` registries generated code references.
//!
//! Every error code a slice or entity raises must be a constant on a registry (doctor rule SKY0018), so the
//! generators reference one and this makes sure it exists: the registry file is created when absent and the
//! constant appended when missing. Re-running never duplicates or clobbers a constant.

use std::path::Path;

use anyhow::Result;

use super::{embedded, text};

/// One constant on a module's error-code registry.
pub struct ErrorCode<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub summary: &'a str,
}

/// Ensures `Modules/<module>/<module>ErrorCodes.cs` declares `code`, creating the registry if needed.
pub fn ensure(module_dir: &Path, namespace: &str, module: &str, code: &ErrorCode) -> Result<()> {
    let path = module_dir.join(format!("{module}ErrorCodes.cs"));
    if !path.exists() {
        let registry = text::fill(
            embedded::dotnet("scaffold/ErrorCodes.cs.cstmpl"),
            &[
                ("__NAMESPACE__", namespace),
                ("__MODULE__", module),
                ("__SUMMARY__", code.summary),
                ("__CONST__", code.name),
                ("__VALUE__", code.value),
            ],
        );
        text::write(&path, registry)?;
        println!("created {}", path.display());
        return Ok(());
    }

    let current = text::read(&path)?;
    if current.contains(&format!("const string {} ", code.name)) {
        return Ok(());
    }
    let Some(last_brace) = current.rfind('}') else {
        return Ok(());
    };
    let nl = text::newline_of(&current);
    let member = format!(
        "{nl}    /// <summary>{}</summary>{nl}    public const string {} = \"{}\";{nl}",
        code.summary, code.name, code.value
    );
    let updated = format!("{}{}{}", &current[..last_brace], member, &current[last_brace..]);
    std::fs::write(&path, updated)?;
    println!("added {module}ErrorCodes.{}", code.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: ErrorCode = ErrorCode {
        name: "IdRequired",
        value: "id.required",
        summary: "The id is required.",
    };

    #[test]
    fn creates_the_registry_then_appends_without_duplicating() {
        let dir = tempfile::tempdir().unwrap();
        ensure(dir.path(), "Acme.Api", "Billing", &CODE).unwrap();
        let other = ErrorCode {
            name: "InvoiceNotFound",
            value: "invoice.not_found",
            summary: "No invoice.",
        };
        ensure(dir.path(), "Acme.Api", "Billing", &other).unwrap();
        ensure(dir.path(), "Acme.Api", "Billing", &CODE).unwrap();

        let registry = std::fs::read_to_string(dir.path().join("BillingErrorCodes.cs")).unwrap();
        assert!(registry.starts_with("namespace Acme.Api.Modules.Billing;\n"));
        assert_eq!(registry.matches("const string IdRequired ").count(), 1);
        assert!(registry.ends_with(
            "    /// <summary>No invoice.</summary>\n    public const string InvoiceNotFound = \"invoice.not_found\";\n}\n"
        ));
    }
}
