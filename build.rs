use std::io::Write;

fn main() {
    compress_dicts();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/abstract.ico");
        res.compile().expect("failed to embed windows icon");
    }
    println!("cargo:rerun-if-changed=assets/abstract.ico");
}

fn compress_dicts() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("dict");
    std::fs::create_dir_all(&out).unwrap();
    for name in ["en_US", "pt_BR"] {
        for ext in ["aff", "dic"] {
            let src = format!("assets/dict/{name}.{ext}");
            println!("cargo:rerun-if-changed={src}");
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            encoder.write_all(&std::fs::read(&src).unwrap()).unwrap();
            std::fs::write(
                out.join(format!("{name}.{ext}.z")),
                encoder.finish().unwrap(),
            )
            .unwrap();
        }
    }
}
