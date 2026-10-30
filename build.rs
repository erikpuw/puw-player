fn main() {
    println!("cargo:rerun-if-changed=ui/player.slint");
    if std::env::var_os("CARGO_FEATURE_GUI").is_some() {
        slint_build::compile("ui/player.slint").expect("compile ui/player.slint");
    }
}
