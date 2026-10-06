fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/abstract.ico");
        res.compile().expect("failed to embed windows icon");
    }
    compress_dicts();
    println!("cargo:rerun-if-changed=assets/abstract.ico");
}

/// `assets/dict/*.aff|.dic` are ~6 MB raw but ~1.6 MB deflated: ship them
/// zlib-compressed and inflate once on the background load.
fn compress_dicts() {
    use std::io::Write;
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("dict");
    std::fs::create_dir_all(&out).unwrap();
    for name in ["en_US", "pt_BR"] {
        for ext in ["aff", "dic"] {
            let src = format!("assets/dict/{name}.{ext}");
            println!("cargo:rerun-if-changed={src}");
            let data = std::fs::read(&src).unwrap();
            let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            enc.write_all(&data).unwrap();
            std::fs::write(out.join(format!("{name}.{ext}.z")), enc.finish().unwrap()).unwrap();
        }
    }
}
