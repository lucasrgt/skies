//! The one shell every runner command goes through, and how a placeholder's value enters a command.
//!
//! Runner commands are POSIX shell (the template's and the sample's are), so they run through `sh -c` on every
//! platform: on Windows that is Git for Windows' `sh` (Git Bash), found on PATH or in Git's install folder, or the
//! shell `SKIES_SHELL` names. `cmd /C` is never used: it would read the same command with different quoting and
//! no `test`, `||`, or `'…'`. A placeholder's value is quoted for the spot of the command it lands in (bare, inside
//! `'…'`, or inside `"…"`), so a path with a space or a quote reaches the tool as one argument.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Result, bail};

/// The variable that names the shell to run runner commands with, on any platform.
pub const SHELL_VAR: &str = "SKIES_SHELL";

/// The shell runner commands run with: `$SKIES_SHELL`, else `sh`, which on Windows must be found (Git Bash).
pub fn program() -> Result<PathBuf> {
    if let Some(shell) = std::env::var_os(SHELL_VAR).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(shell));
    }
    if !cfg!(windows) {
        return Ok(PathBuf::from("sh"));
    }
    let on_path = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|dir| dir.join("sh.exe"));
    let git = ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .flat_map(|base| {
            let base = PathBuf::from(base);
            [base.join("Git/bin/sh.exe"), base.join("Programs/Git/bin/sh.exe")]
        });
    match on_path.chain(git).find(|candidate| candidate.is_file()) {
        Some(sh) => Ok(sh),
        None => bail!(
            "runner commands are POSIX shell and run through `sh -c`, and no `sh` was found. Install Git for Windows \
             (its Git Bash provides sh.exe) and put its bin folder on PATH, or set {SHELL_VAR} to a POSIX shell."
        ),
    }
}

/// Where a placeholder sits in the command, which decides how its value is quoted.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Bare,
    Single,
    Double,
}

/// Replaces `{name}` for every known placeholder, its value quoted for where it sits; unknown braces are left alone
/// so shell syntax like `${VAR}` or JSON in a command survives.
pub fn expand(template: &str, values: &BTreeMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut context = Context::Bare;
    let mut rest = template;
    while let Some(ch) = rest.chars().next() {
        if ch == '{'
            && let Some((key, value)) = values
                .iter()
                .find(|(key, _)| rest[1..].starts_with(&format!("{key}}}")))
        {
            out.push_str(&quote(value, context));
            rest = &rest[key.len() + 2..];
            continue;
        }
        let len = ch.len_utf8();
        let mut taken = len;
        match (context, ch) {
            (Context::Bare, '\'') => context = Context::Single,
            (Context::Single, '\'') => context = Context::Bare,
            (Context::Bare, '"') => context = Context::Double,
            (Context::Double, '"') => context = Context::Bare,
            // A backslash outside single quotes escapes the next character, which then changes no quoting.
            (Context::Bare | Context::Double, '\\') => {
                taken += rest[len..].chars().next().map_or(0, char::len_utf8);
            }
            _ => {}
        }
        out.push_str(&rest[..taken]);
        rest = &rest[taken..];
    }
    out
}

/// `value` as one word at its spot: wrapped in `'…'` when bare, with `'` spelled `'\''` inside single quotes, and
/// with `\`, `"`, `$`, and `` ` `` escaped inside double quotes.
fn quote(value: &str, context: Context) -> String {
    match context {
        Context::Bare => format!("'{}'", value.replace('\'', r"'\''")),
        Context::Single => value.replace('\'', r"'\''"),
        Context::Double => value.chars().fold(String::new(), |mut out, ch| {
            if matches!(ch, '\\' | '"' | '$' | '`') {
                out.push('\\');
            }
            out.push(ch);
            out
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn values() -> BTreeMap<&'static str, String> {
        BTreeMap::from([
            ("id", "0012".to_string()),
            ("report", "/tmp/a dir/it's \"r\" $x.xml".to_string()),
        ])
    }

    #[test]
    fn expands_known_placeholders_only() {
        assert_eq!(
            expand("test --filter S{id}. ${HOME} {\"a\":1} {unknown}", &values()),
            "test --filter S'0012'. ${HOME} {\"a\":1} {unknown}"
        );
    }

    /// Each spot a placeholder can sit in reaches the program as the exact value, one argument.
    #[cfg(unix)]
    #[test]
    fn a_value_with_spaces_and_quotes_stays_one_argument_wherever_it_sits() {
        let report = &values()["report"];
        for (template, expected) in [
            ("printf '%s|' {report}", format!("{report}|")),
            ("printf '%s|' --out={report}", format!("--out={report}|")),
            (
                "printf '%s|' 'trx;LogFileName={report}'",
                format!("trx;LogFileName={report}|"),
            ),
            (
                "printf '%s|' \"trx;LogFileName={report}\"",
                format!("trx;LogFileName={report}|"),
            ),
            ("printf '%s|' \\'{report}", format!("'{report}|")),
        ] {
            let command = expand(template, &values());
            let output = Command::new("sh").args(["-c", &command]).output().unwrap();
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                expected,
                "{template} -> {command}"
            );
        }
    }
}
