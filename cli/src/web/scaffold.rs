//! File-writing and process helpers shared by the React and Flutter scaffolders.
//!
//! Two write policies exist on purpose. Generated units (`write_new`) refuse to overwrite anything, because a
//! half-written unit next to a hand-edited one is worse than no unit. Hand-owned seams (`write_missing`) are
//! written once and then belong to the application, so later runs skip them instead of failing.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use minijinja::Environment;
use serde::Serialize;

/// Renders an embedded template. Trailing newlines are kept so emitted files end the way the template does.
pub fn render(template: &str, context: impl Serialize) -> Result<String> {
    let mut env = Environment::new();
    env.set_keep_trailing_newline(true);
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    env.render_str(template, context)
        .context("rendering an embedded template")
}

/// Writes every file or none: any existing path aborts before the first write.
pub fn write_new(files: &[(PathBuf, String)]) -> Result<()> {
    let collisions: Vec<String> = files
        .iter()
        .filter(|(path, _)| path.exists())
        .map(|(path, _)| path.display().to_string())
        .collect();
    if !collisions.is_empty() {
        bail!("refusing to overwrite {}", collisions.join(", "));
    }
    for (path, contents) in files {
        write(path, contents)?;
        println!("created {}", path.display());
    }
    Ok(())
}

/// Writes the files that do not exist yet and reports the ones left alone.
pub fn write_missing(files: &[(PathBuf, String)]) -> Result<()> {
    for (path, contents) in files {
        if path.exists() {
            println!("kept {} (hand-owned)", path.display());
        } else {
            write(path, contents)?;
            println!("created {}", path.display());
        }
    }
    Ok(())
}

fn write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

/// `to` expressed relative to the directory `from`, with forward slashes and a leading `./` so it reads as a
/// relative import or config path on every platform. Both paths should already be absolute. Paths that share
/// nothing but the root stay absolute: a climb all the way to `/` says less than the absolute path does.
pub fn relative_path(from_dir: &Path, target: &Path) -> String {
    let from: Vec<Component> = from_dir.components().collect();
    let to: Vec<Component> = target.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    if !to[..common].iter().any(|c| matches!(c, Component::Normal(_))) {
        return target.to_string_lossy().replace('\\', "/");
    }
    let mut parts: Vec<String> = vec!["..".to_string(); from.len() - common];
    parts.extend(
        to[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    let joined = parts.join("/");
    if joined.starts_with("..") {
        joined
    } else {
        format!("./{joined}")
    }
}

/// Runs a tool to completion with inherited output. A missing executable gets its own message because it is
/// the most common failure and the OS error ("No such file or directory") does not name the tool.
pub fn run(program: &str, args: &[&str], cwd: &Path) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("`{program}` is not on PATH")
            } else {
                anyhow::Error::new(error).context(format!("starting {program}"))
            }
        })?;
    if !status.success() {
        bail!("`{program} {}` failed in {} ({status})", args.join(" "), cwd.display());
    }
    Ok(())
}

/// Resolves `--package` (or the working directory) to an absolute directory.
pub fn package_dir(package: Option<&Path>) -> Result<PathBuf> {
    let dir = match package {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()?,
    };
    dir.canonicalize()
        .with_context(|| format!("package directory {} does not exist", dir.display()))
}

/// A content hash of a directory tree, used only to tell "regenerated identically" from "the contract moved".
/// `None` when the directory does not exist yet.
pub fn tree_hash(dir: &Path) -> Option<u64> {
    fn visit(dir: &Path, root: &Path, hasher: &mut DefaultHasher) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for path in paths {
            path.strip_prefix(root).unwrap_or(&path).hash(hasher);
            if path.is_dir() {
                visit(&path, root, hasher);
            } else if let Ok(bytes) = std::fs::read(&path) {
                bytes.hash(hasher);
            }
        }
    }
    if !dir.is_dir() {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    visit(dir, dir, &mut hasher);
    Some(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_climb_and_descend() {
        let from = Path::new("/app/web/src/i18n");
        assert_eq!(
            relative_path(from, Path::new("/app/web/src/items/items.i18n")),
            "../items/items.i18n"
        );
        assert_eq!(
            relative_path(Path::new("/app/web"), Path::new("/app/web/src/x")),
            "./src/x"
        );
        assert_eq!(
            relative_path(Path::new("/tmp/web"), Path::new("/home/api/c.json")),
            "/home/api/c.json"
        );
    }

    #[test]
    fn write_new_refuses_before_writing_anything() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("b.ts");
        std::fs::write(&existing, "mine").unwrap();
        let files = vec![
            (dir.path().join("a.ts"), "a".to_string()),
            (existing.clone(), "b".to_string()),
        ];

        let error = write_new(&files).unwrap_err().to_string();

        assert!(error.contains("refusing to overwrite"));
        assert!(!dir.path().join("a.ts").exists());
        assert_eq!(std::fs::read_to_string(existing).unwrap(), "mine");
    }

    #[test]
    fn render_fails_on_an_unknown_placeholder() {
        assert!(render("{{ missing }}", minijinja::context! {}).is_err());
        assert_eq!(
            render("a {{ x }}\n", minijinja::context! { x => "b" }).unwrap(),
            "a b\n"
        );
    }

    #[test]
    fn tree_hash_sees_content_changes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(tree_hash(&dir.path().join("missing")), None);
        std::fs::write(dir.path().join("a.ts"), "1").unwrap();
        let first = tree_hash(dir.path());
        std::fs::write(dir.path().join("a.ts"), "2").unwrap();
        assert_ne!(first, tree_hash(dir.path()));
    }
}
