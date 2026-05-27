fn main() {
    let icons_dir = std::path::Path::new("icons");
    if let Ok(entries) = std::fs::read_dir(icons_dir) {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
    println!("cargo:rerun-if-changed=tauri.conf.json");
    tauri_build::build()
}
