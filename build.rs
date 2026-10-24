use std::path::Path;
use std::process::Command;

fn main() {
    fastframe_i18n::build::compile_catalogs("assets/i18n");
    windows_icon();
}

/// Embeds packaging/icons/youtfast.ico in the Windows executable: Explorer,
/// shortcuts and the taskbar show the file's own icon, not the window's.
///
/// windres is called directly. Crates that wrap it pick the resource
/// compiler by the build script's host, and the Windows runners' host is
/// MSVC while the target is GNU: rc.exe's output does not link with ld.
fn windows_icon() {
    let var = |name| std::env::var(name).unwrap_or_default();
    if var("CARGO_CFG_TARGET_OS") != "windows" {
        return;
    }
    if var("CARGO_CFG_TARGET_ENV") != "gnu" {
        println!("cargo:warning=The icon is only embedded for the GNU target.");
        return;
    }
    let icon = Path::new(&var("CARGO_MANIFEST_DIR")).join("packaging/icons/youtfast.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-env-changed=WINDRES");
    let out = Path::new(&var("OUT_DIR")).to_owned();
    let script = out.join("youtfast.rc");
    // An absolute path, so windres's idea of relative paths does not matter;
    // '/' needs no escaping, and the code page covers non-ASCII directories.
    let icon = icon.display().to_string().replace('\\', "/");
    std::fs::write(
        &script,
        format!("#pragma code_page(65001)\n1 ICON \"{icon}\"\n"),
    )
    .expect("Could not write the resource script");
    let object = out.join("youtfast-icon.o");
    let arch = var("CARGO_CFG_TARGET_ARCH");
    // The object must be the target's machine, whichever windres answers.
    let machine = match arch.as_str() {
        "x86_64" => Some("--target=pe-x86-64"),
        "x86" => Some("--target=pe-i386"),
        _ => None,
    };
    let prefixed = format!("{arch}-w64-mingw32-windres");
    let windres = std::env::var("WINDRES").ok();
    let candidates = windres
        .iter()
        .map(String::as_str)
        .chain([prefixed.as_str(), "windres"]);
    let built = candidates.into_iter().any(|windres| {
        Command::new(windres)
            .args(machine)
            .arg("--input")
            .arg(&script)
            .args(["--input-format=rc", "--output-format=coff", "--output"])
            .arg(&object)
            .status()
            .is_ok_and(|status| status.success())
    });
    assert!(
        built,
        "windres could not compile the icon; install MinGW's binutils or set WINDRES"
    );
    println!("cargo:rustc-link-arg-bins={}", object.display());
}
