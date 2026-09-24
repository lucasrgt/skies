//! Rebuild when a template changes. The templates are embedded with `include_dir!`, which cannot tell Cargo about
//! the files it reads on stable Rust, so without this an edited template would ship stale until the next source edit.

fn main() {
    println!("cargo:rerun-if-changed=templates");
}
