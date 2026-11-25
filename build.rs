fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for name in ["CIE_xyz_1931_2deg.csv","CIE_xyz_1964_10deg.csv","CIE_std_illum_D50.csv","CIE_std_illum_D65.csv","CIE_std_illum_A_1nm.csv"] {
        let source = std::path::Path::new("assets/cie").join(name);
        println!("cargo:rerun-if-changed={}", source.display());
        let bytes = match std::fs::read(&source) {
            Ok(bytes) => bytes,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => panic!("CIE data read failed: {e}"),
        };
        std::fs::write(out.join(name),bytes).expect("Copy CIE table");
    }
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
    resource.set("FileDescription","ICC Profile Studio");
    resource.set("ProductName","ICC Profile Studio");
    resource.set("InternalName","ICC Profile Studio");
    resource.set("OriginalFilename","ICC-Profile-Studio.exe");
    resource.set("CompanyName","emadghasemi.ir");
    let version=include_str!("STUDIO_VERSION").trim();
    println!("cargo:rerun-if-changed=STUDIO_VERSION");
    resource.set("ProductVersion",version);
    resource.set("FileVersion",version);
    resource.compile().expect("Embed Windows application icon");
}
