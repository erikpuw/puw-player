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
        // Version info (Properties → Details) from Cargo.toml; an exe without it looks more
        // suspicious to antivirus heuristics.
        let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("version.res");
        std::fs::write(&out, version_res()).expect("write version.res");
        println!("cargo:rustc-link-arg-bins={}", out.display());
    }
}

/// A .res file holding one RT_VERSION resource (VS_VERSIONINFO).
fn version_res() -> Vec<u8> {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (major, minor, patch) = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    let ms = (major << 16) | minor;
    let ls = patch << 16;
    let mut fixed = Vec::new();
    // VS_FIXEDFILEINFO: signature, struct version, file/product version, flags mask, flags,
    // VOS_NT_WINDOWS32, VFT_APP, subtype, date.
    for v in [0xFEEF04BD, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0] {
        fixed.extend_from_slice(&u32::to_le_bytes(v));
    }
    let text = |key: &str, value: &str| node(key, &utf16z(value), 1, &[]);
    let strings = [
        text("CompanyName", "erikpuw"),
        text("FileDescription", "Puw Player"),
        text("FileVersion", version),
        text("InternalName", "puw-player"),
        text("LegalCopyright", "Copyright (c) 2026 erikpuw"),
        text("OriginalFilename", "puw-player.exe"),
        text("ProductName", "Puw Player"),
        text("ProductVersion", version),
    ];
    let table = node("040904B0", &[], 1, &strings);
    let translation = node("Translation", &[0x09, 0x04, 0xB0, 0x04], 0, &[]);
    let info = node(
        "VS_VERSION_INFO",
        &fixed,
        0,
        &[node("StringFileInfo", &[], 1, &[table]), node("VarFileInfo", &[], 1, &[translation])],
    );
    // Leading empty resource header, then RT_VERSION (16) id 1, en-US.
    let mut res = vec![0u8, 0, 0, 0, 32, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0];
    res.extend_from_slice(&[0; 16]);
    let mut hdr = vec![0xFF, 0xFF, 16, 0, 0xFF, 0xFF, 1, 0];
    hdr.extend_from_slice(&0u32.to_le_bytes()); // data version
    hdr.extend_from_slice(&0x0030u16.to_le_bytes()); // memory flags
    hdr.extend_from_slice(&0x0409u16.to_le_bytes()); // language
    hdr.extend_from_slice(&[0; 8]); // version, characteristics
    res.extend_from_slice(&(info.len() as u32).to_le_bytes());
    res.extend_from_slice(&(8 + hdr.len() as u32).to_le_bytes());
    res.extend_from_slice(&hdr);
    res.extend_from_slice(&info);
    pad4(&mut res);
    res
}

/// One version-info block: wLength, wValueLength, wType, key, value, children, each 4-aligned.
/// wValueLength counts WORDs for text values (wType 1) and bytes for binary ones.
fn node(key: &str, value: &[u8], kind: u16, children: &[Vec<u8>]) -> Vec<u8> {
    let value_len = if kind == 1 { value.len() / 2 } else { value.len() };
    let mut b = vec![0, 0];
    b.extend_from_slice(&(value_len as u16).to_le_bytes());
    b.extend_from_slice(&kind.to_le_bytes());
    b.extend_from_slice(&utf16z(key));
    pad4(&mut b);
    b.extend_from_slice(value);
    for child in children {
        pad4(&mut b);
        b.extend_from_slice(child);
    }
    let len = b.len() as u16;
    b[..2].copy_from_slice(&len.to_le_bytes());
    b
}

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

fn pad4(b: &mut Vec<u8>) {
    b.resize(b.len().next_multiple_of(4), 0);
}
