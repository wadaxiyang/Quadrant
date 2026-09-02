//! `SQLite` persistence adapter for Quadrant.

#![forbid(unsafe_code)]

mod backup;
mod connection;
mod focus;
mod mapping;
mod migrations;
mod review;
mod store;

pub use backup::{AppliedRestore, apply_pending_restore};
pub use store::SqliteStore;
