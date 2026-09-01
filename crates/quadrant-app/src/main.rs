//! Quadrant composition root.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use quadrant_application::{SystemThemeSource, ThemeMode};

    let theme_source = quadrant_platform::PlatformThemeSource;
    let config = quadrant_ui::UiShellConfig {
        theme_mode: ThemeMode::System,
        system_theme: theme_source.current_theme(),
    };

    quadrant_ui::run(config, |_| {})?;
    Ok(())
}
