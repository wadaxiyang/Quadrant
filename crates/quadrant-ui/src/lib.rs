//! Slint presentation adapter. Business, storage, and platform work stays outside this crate.

// Slint's generated module contains narrowly scoped unsafe implementation code.
#![deny(unsafe_code)]
// Public generated component accessors do not carry Rust doc comments.
#![allow(missing_docs)]

slint::include_modules!();

mod shell;

pub use shell::{UiShellConfig, run};
