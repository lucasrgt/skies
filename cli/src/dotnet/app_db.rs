//! The app's `AppDb`: finding the file that declares it and registering an entity's `DbSet` there.
//!
//! Generated slices take the one DbContext every module shares as `AppDb`, so a `DbSet` is the line that makes an
//! entity queryable. `g crud` adds it itself, below the last `DbSet` property the class already declares (the
//! anchor every `AppDb` with an entity has), and prints the exact line when there is none; `g entity` names the
//! file to add it to.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::text;

static DECLARATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bclass\s+AppDb\b").expect("AppDb declaration regex"));

/// A `public DbSet<T> Name => Set<T>();` property on a line of its own: the anchor a new set goes below.
static DB_SET_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*public\s+DbSet<[^>]+>\s+[A-Za-z_][A-Za-z0-9_]*\s*=>\s*Set<[^>]+>\(\);[ \t]*\r?$")
        .expect("DbSet line regex")
});

/// Directories that hold build output or tool-emitted code, never the app's DbContext.
const SKIPPED: &[&str] = &["bin", "obj", "Migrations", "node_modules"];

/// The file under the API project that declares `class AppDb`, if any.
pub fn locate(root: &Path) -> Result<Option<PathBuf>> {
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if path.is_dir() {
                if !SKIPPED.contains(&name.as_str()) && !name.starts_with('.') {
                    walk(&path, found)?;
                }
            } else if name.ends_with(".cs") && DECLARATION.is_match(&text::read(&path)?) {
                found.push(path);
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    walk(root, &mut found)?;
    found.sort();
    Ok(found.into_iter().next())
}

/// The name of the `DbSet<entity>` the source already declares (`Users` for `User`), qualified or not.
pub fn existing_set(source: &str, entity: &str) -> Option<String> {
    let pattern = format!(
        r"DbSet<(?:[A-Za-z0-9_]+\.)*{}>\s+(?<plural>[A-Za-z_][A-Za-z0-9_]*)\s*=>",
        regex::escape(entity)
    );
    Regex::new(&pattern)
        .expect("DbSet regex")
        .captures(source)
        .map(|capture| capture["plural"].to_string())
}

/// What [`register`] did.
#[derive(Debug, PartialEq)]
pub enum Registration {
    /// The set was already declared under this name.
    Present(String),
    /// The set was added under this name.
    Added(String),
    /// No anchor: the owner adds these lines by hand.
    Manual(Vec<String>),
}

/// Declares `public DbSet<entity> <Plural> => Set<entity>();` below the last `DbSet` property, with the
/// module's `using` when the file lacks it. Idempotent: a set already declared is reported, not duplicated.
pub fn register(path: &Path, namespace: &str, module: &str, entity: &str) -> Result<Registration> {
    let source = text::read(path)?;
    if let Some(name) = existing_set(&source, entity) {
        return Ok(Registration::Present(name));
    }
    let plural = text::plural(entity);
    let property = format!("public DbSet<{entity}> {plural} => Set<{entity}>();");
    let using = format!("using {namespace}.Modules.{module};");
    let Some(last) = DB_SET_LINE.find_iter(&source).last() else {
        return Ok(Registration::Manual(vec![using, property]));
    };

    let nl = text::newline_of(&source);
    let line_end = last.end() - usize::from(last.as_str().ends_with('\r'));
    let mut updated = format!("{}{nl}{nl}    {property}{}", &source[..line_end], &source[line_end..]);
    if !source.contains(&using) {
        updated = add_using(&updated, &using, nl);
    }
    std::fs::write(path, updated)?;
    Ok(Registration::Added(plural))
}

/// Adds `using` after the file's last top-level `using` line, or first when it has none.
fn add_using(source: &str, using: &str, nl: &str) -> String {
    let mut offset = 0;
    let mut after_last = None;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("using ") && trimmed.ends_with(';') {
            after_last = Some(offset + line.len());
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            break;
        }
        offset += line.len();
    }
    match after_last {
        Some(at) => format!("{}{using}{nl}{}", &source[..at], &source[at..]),
        None => format!("{using}{nl}{source}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP_DB: &str = concat!(
        "using Acme.Api.Modules.Account;\nusing Microsoft.EntityFrameworkCore;\n\nnamespace Acme.Api;\n\n",
        "public class AppDb(DbContextOptions<AppDb> options) : DbContext(options)\n{\n",
        "    public DbSet<User> Users => Set<User>();\n\n",
        "    public DbSet<UserSession> UserSessions => Set<UserSession>();\n\n",
        "    protected override void OnModelCreating(ModelBuilder model) { }\n}\n",
    );

    fn project(app_db: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Data")).unwrap();
        std::fs::create_dir_all(dir.path().join("obj")).unwrap();
        std::fs::write(dir.path().join("obj/AppDb.cs"), "class AppDb { }").unwrap();
        std::fs::write(dir.path().join("Data/AppDb.cs"), app_db).unwrap();
        dir
    }

    #[test]
    fn finds_the_file_declaring_app_db_wherever_it_lives() {
        let dir = project(APP_DB);
        assert_eq!(locate(dir.path()).unwrap(), Some(dir.path().join("Data/AppDb.cs")));
        assert_eq!(locate(tempfile::tempdir().unwrap().path()).unwrap(), None);
    }

    #[test]
    fn registers_below_the_last_set_with_the_module_using_once() {
        let dir = project(APP_DB);
        let path = dir.path().join("Data/AppDb.cs");

        let first = register(&path, "Acme.Api", "Billing", "Invoice").unwrap();
        let second = register(&path, "Acme.Api", "Billing", "Invoice").unwrap();

        assert_eq!(first, Registration::Added("Invoices".into()));
        assert_eq!(second, Registration::Present("Invoices".into()));
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.starts_with(
            "using Acme.Api.Modules.Account;\nusing Microsoft.EntityFrameworkCore;\nusing Acme.Api.Modules.Billing;\n\n"
        ));
        assert!(source.contains(concat!(
            "    public DbSet<UserSession> UserSessions => Set<UserSession>();\n\n",
            "    public DbSet<Invoice> Invoices => Set<Invoice>();\n\n",
            "    protected override"
        )));
    }

    #[test]
    fn crlf_files_stay_crlf() {
        let dir = project(&APP_DB.replace('\n', "\r\n"));
        let path = dir.path().join("Data/AppDb.cs");
        register(&path, "Acme.Api", "Catalog", "Category").unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.contains("public DbSet<Category> Categories => Set<Category>();\r\n"));
        assert!(!source.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn without_an_anchor_the_owner_gets_the_exact_lines() {
        let dir = project("namespace Acme.Api;\n\npublic class AppDb : DbContext\n{\n}\n");
        let path = dir.path().join("Data/AppDb.cs");
        assert_eq!(
            register(&path, "Acme.Api", "Billing", "Invoice").unwrap(),
            Registration::Manual(vec![
                "using Acme.Api.Modules.Billing;".into(),
                "public DbSet<Invoice> Invoices => Set<Invoice>();".into(),
            ])
        );
    }
}
