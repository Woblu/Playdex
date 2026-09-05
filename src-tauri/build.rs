fn main() {
    // Tauri embeds the window and taskbar icon at build time, but its build
    // script only re-runs when tauri.conf.json changes. Without this, freshly
    // generated icons are silently ignored and the binary keeps the old one
    // until some unrelated change forces a rebuild.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
