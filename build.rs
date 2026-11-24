fn main() {
    println!("cargo:rerun-if-changed=assets/icon.png");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") { return; }
    // Windows ICO directory containing the original transparent 96x96 PNG.
    let png = include_bytes!("assets/icon.png");
    let mut ico = vec![0,0,1,0,1,0,96,96,0,0,1,0,32,0];
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(png);
    let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("app.ico");
    std::fs::write(&path, ico).unwrap();
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(path.to_str().unwrap());
    resource.set("FileDescription","ICC Profile Rebuilder");
    resource.set("ProductName","ICC Profile Rebuilder");
    resource.compile().expect("Embed Windows application icon");
}
