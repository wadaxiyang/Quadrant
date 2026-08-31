//! Slint presentation adapter. Business, storage, and platform work stays outside this crate.

// Slint's generated module contains narrowly scoped unsafe implementation code.
#![deny(unsafe_code)]
// Public generated component accessors do not carry Rust doc comments.
#![allow(missing_docs)]

slint::include_modules!();

/// Constructs the main window and runs the Slint event loop.
///
/// # Errors
///
/// Returns a platform error when the window or event loop cannot be created.
pub fn run() -> Result<(), slint::PlatformError> {
    MainWindow::new()?.run()
}
