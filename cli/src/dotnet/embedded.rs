//! The templates compiled into the binary.
//!
//! `skies` ships as one file (a GitHub release asset or an npm platform package), so the templates cannot live
//! next to it the way the 4.x dotnet tool kept them. `include_dir` embeds `cli/templates/app` (the `skies new`
//! solution) and `cli/templates/dotnet` (the generator blueprints) at build time.

use include_dir::{Dir, File, include_dir};

static APP: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/templates/app");
static DOTNET: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/templates/dotnet");

/// Every file of the `skies new` template, as (path relative to the template root, bytes).
pub fn app_files() -> Vec<(String, &'static [u8])> {
    let mut files = Vec::new();
    collect(&APP, &mut files);
    files
        .into_iter()
        .map(|file| (slash_path(file), file.contents()))
        .collect()
}

/// A single generator template by its path under `templates/dotnet`, e.g. `scaffold/Slice.cs.cstmpl`.
///
/// A missing template is a packaging bug, not a user error, so it panics with the path.
pub fn dotnet(path: &str) -> &'static str {
    DOTNET
        .get_file(path)
        .and_then(File::contents_utf8)
        .unwrap_or_else(|| panic!("embedded template {path} is missing or not UTF-8"))
}

/// Every template under `templates/dotnet/<folder>`, as (path relative to that folder, contents), ordered
/// ordinally so generators print and write in a stable order.
pub fn dotnet_folder(folder: &str) -> Vec<(String, &'static str)> {
    let dir = DOTNET
        .get_dir(folder)
        .unwrap_or_else(|| panic!("embedded template folder {folder} is missing"));
    let mut files = Vec::new();
    collect(dir, &mut files);
    let prefix = format!("{folder}/");
    let mut out: Vec<(String, &'static str)> = files
        .into_iter()
        .map(|file| {
            let path = slash_path(file);
            let relative = path.strip_prefix(&prefix).unwrap_or(&path).to_string();
            let text = file
                .contents_utf8()
                .unwrap_or_else(|| panic!("template {path} is not UTF-8"));
            (relative, text)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn collect(dir: &'static Dir<'static>, files: &mut Vec<&'static File<'static>>) {
    files.extend(dir.files());
    for child in dir.dirs() {
        collect(child, files);
    }
}

fn slash_path(file: &File<'_>) -> String {
    file.path().to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_template_carries_dotfolders() {
        let files = app_files();
        assert!(files.iter().any(|(path, _)| path == ".template.config/template.json"));
        assert!(files.iter().any(|(path, _)| path == ".specs/README.md"));
    }

    #[test]
    fn folders_list_relative_paths_in_ordinal_order() {
        let auth = dotnet_folder("auth");
        assert!(
            auth.iter()
                .any(|(path, _)| path == "Modules/Account/Slices/Login.cs.cstmpl")
        );
        assert!(auth.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
}
