fn main() {
    println!("cargo:rerun-if-changed=../foco.svg");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    if let Err(error) = build_windows_resources() {
        panic!("failed to build Windows resources from foco.svg: {error}");
    }
}

fn build_windows_resources() -> Result<(), Box<dyn std::error::Error>> {
    let svg_path = std::path::Path::new("../foco.svg");
    let output_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let icon_path = output_dir.join("foco.ico");

    write_icon_file(svg_path, &icon_path)?;

    let icon_path = icon_path
        .to_str()
        .ok_or("generated icon path must be valid UTF-8")?;

    winresource::WindowsResource::new()
        .set_icon(icon_path)
        .set("ProductName", "Foco")
        .set("FileDescription", "Foco")
        .set("InternalName", "foco.exe")
        .set("OriginalFilename", "foco.exe")
        .compile()?;

    Ok(())
}

fn write_icon_file(
    svg_path: &std::path::Path,
    icon_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    const ICON_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

    if !svg_path.is_file() {
        return Err(format!("missing app icon source at {}", svg_path.display()).into());
    }

    let svg = std::fs::read(svg_path)?;
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&svg, &options)?;
    let svg_size = tree.size();
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for icon_size in ICON_SIZES {
        let mut pixmap = resvg::tiny_skia::Pixmap::new(icon_size, icon_size)
            .ok_or("failed to allocate icon pixmap")?;
        let transform = resvg::tiny_skia::Transform::from_scale(
            icon_size as f32 / svg_size.width(),
            icon_size as f32 / svg_size.height(),
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let image = ico::IconImage::from_rgba_data(icon_size, icon_size, pixmap.take());
        icon_dir.add_entry(ico::IconDirEntry::encode(&image)?);
    }

    let icon_file = std::fs::File::create(icon_path)?;
    icon_dir.write(icon_file)?;

    Ok(())
}
