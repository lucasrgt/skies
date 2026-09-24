//! The module context docs a change reaches: `**/Modules/<M>/…` → `**/Modules/<M>/<M>.ctx.md`.
//!
//! A ctx.md carries the invariants of a module and cites the specs that prove them (`0002-withdraw#FM-2`). It is
//! kept alive by being read and revised next to the proof, never by a gate: `proof impact` names the ctx files to
//! read before writing failure modes, and `proof record` notes, without failing, when a spec touched a module whose
//! ctx was not revised in the same change. A ctx is prose kept fresh by citation resolution (SKY0005), so it is not
//! hashed into a footprint unless a spec lists it in `touches`.

use std::collections::BTreeSet;
use std::path::Path;

/// The suffix every module context doc carries.
const CTX_SUFFIX: &str = ".ctx.md";

/// Whether `path` (project-relative) is a module context doc.
pub fn is_ctx(path: &str) -> bool {
    path.ends_with(CTX_SUFFIX)
}

/// The ctx.md of the module `path` sits in (a file or a folder under `Modules/<M>/`, or that folder itself), by
/// convention only; whether it exists is the caller's question. The innermost `Modules/` segment wins.
pub fn module_ctx(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    let at = parts[..parts.len().saturating_sub(1)]
        .iter()
        .rposition(|part| *part == "Modules")?;
    let module = parts[at + 1];
    Some(format!("{}/{module}/{module}{CTX_SUFFIX}", parts[..=at].join("/")))
}

/// The existing ctx files of the modules `paths` reach, sorted. A path outside `Modules/<M>/`, or a module without
/// a ctx (a frontend feature folder, say), contributes nothing.
pub fn touched<'a>(root: &Path, paths: impl IntoIterator<Item = &'a String>) -> BTreeSet<String> {
    paths
        .into_iter()
        .filter_map(|path| module_ctx(path))
        .filter(|ctx| root.join(ctx).is_file())
        .collect()
}

/// The file name of a ctx path, for a short note (`Wallets.ctx.md`).
pub fn file_name(ctx: &str) -> &str {
    ctx.rsplit('/').next().unwrap_or(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_under_a_module_maps_to_that_modules_ctx() {
        assert_eq!(
            module_ctx("backend/Api/Modules/Wallets/Slices/Deposit.cs").as_deref(),
            Some("backend/Api/Modules/Wallets/Wallets.ctx.md")
        );
        assert_eq!(
            module_ctx("Modules/Wallets/Wallet.cs").as_deref(),
            Some("Modules/Wallets/Wallets.ctx.md")
        );
        assert_eq!(
            module_ctx("backend/Api/Modules/Wallets").as_deref(),
            Some("backend/Api/Modules/Wallets/Wallets.ctx.md")
        );
        assert_eq!(
            module_ctx("backend/Api/Modules/Wallets/Wallets.ctx.md").as_deref(),
            Some("backend/Api/Modules/Wallets/Wallets.ctx.md")
        );
        assert_eq!(module_ctx("backend/Api/Modules"), None);
        assert_eq!(module_ctx("backend/Api/Program.cs"), None);
        assert_eq!(module_ctx("frontend/web/src/features/deposit/Deposit.tsx"), None);
    }

    #[test]
    fn only_existing_ctx_files_are_touched() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("Api/Modules/Wallets")).unwrap();
        std::fs::write(root.path().join("Api/Modules/Wallets/Wallets.ctx.md"), "# wallets\n").unwrap();
        let paths = [
            "Api/Modules/Wallets/Wallet.cs".to_string(),
            "Api/Modules/Wallets/Slices/Deposit.cs".to_string(),
            "web/Modules/Screens/Deposit.tsx".to_string(),
        ];
        assert_eq!(
            touched(root.path(), &paths).into_iter().collect::<Vec<_>>(),
            ["Api/Modules/Wallets/Wallets.ctx.md"]
        );
        assert_eq!(file_name("Api/Modules/Wallets/Wallets.ctx.md"), "Wallets.ctx.md");
        assert!(is_ctx("Api/Modules/Wallets/Wallets.ctx.md"));
        assert!(!is_ctx("Api/Modules/Wallets/Wallet.cs"));
    }
}
