fn main() {
    // `generate_context!()` embeds the icons at compile time, but cargo doesn't track them, so a
    // new icon set would otherwise keep the old icon in dev builds until something else changes.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
