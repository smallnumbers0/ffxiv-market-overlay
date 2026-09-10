fn main() {
    ensure_catalog_placeholder();
    tauri_build::build()
}

/// `resources/items.db` is a generated artifact (see `npm run sync-catalog`)
/// and is not committed, but `tauri-build` fails outright on a bundle resource
/// that doesn't exist. Drop an empty placeholder in so a fresh clone compiles -
/// including the `sync_catalog` binary that produces the real one.
///
/// An empty file is not a usable catalog, so the app treats it as "not synced
/// yet" and shows the setup prompt rather than an empty search box.
fn ensure_catalog_placeholder() {
    let path = std::path::Path::new("resources/items.db");
    println!("cargo:rerun-if-changed=resources/items.db");
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, b"");
}
