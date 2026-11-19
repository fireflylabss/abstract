fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/abstract.ico");
        res.compile().expect("failed to embed windows icon");
    }
    println!("cargo:rerun-if-changed=assets/abstract.ico");
}
