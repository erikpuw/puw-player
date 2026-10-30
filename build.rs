fn main() {
    println!("cargo:rerun-if-changed=ui/player.slint");
    if std::env::var_os("CARGO_FEATURE_GUI").is_some() {
        slint_build::compile("ui/player.slint").expect("compile ui/player.slint");
    }
    // App icon in the exe: assets/puw.res (made by assets/make_icon.py) is a compiled resource
    // file the MSVC linker takes as-is.
    println!("cargo:rerun-if-changed=assets/puw.res");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && msvc {
        let res = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("assets")
            .join("puw.res");
        println!("cargo:rustc-link-arg-bins={}", res.display());
    }
}
