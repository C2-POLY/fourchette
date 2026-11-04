//! On macOS, links `assets/Info.plist` into the binary so the menu bar says "Fourchette" (with
//! its capital) when the app runs straight from `target/`, outside an .app bundle.
fn main() {
    let plist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/Info.plist");
    println!("cargo:rerun-if-changed={}", plist.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}", plist.display());
    }
}
