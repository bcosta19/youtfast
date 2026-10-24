use std::path::Path;

fn main() {
    fastframe_i18n::build::compile_catalogs("assets/i18n");
    windows_icon();
}

/// Embeds packaging/icons/youtfast.ico in the Windows executable: Explorer,
/// shortcuts and the taskbar show the file's own icon, not the window's.
fn windows_icon() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR");
    let icon = Path::new(&manifest).join("packaging/icons/youtfast.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    // Written with an absolute path, so the resource compiler's own idea of
    // relative paths does not matter; both rc and windres accept '/'.
    let script =
        Path::new(&std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR")).join("youtfast.rc");
    let icon = icon.display().to_string().replace('\\', "/");
    std::fs::write(&script, format!("1 ICON \"{icon}\"\n"))
        .expect("Could not write the resource script");
    embed_resource::compile(&script, embed_resource::NONE)
        .manifest_optional()
        .expect("Could not embed the Windows icon");
}
