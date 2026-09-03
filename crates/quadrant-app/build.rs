//! Embeds Windows application identity resources into the composition-root executable.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let icon = "../../assets/branding/quadrant.ico";
    println!("cargo:rerun-if-changed={icon}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon(icon)
            .compile()?;
    }

    Ok(())
}
