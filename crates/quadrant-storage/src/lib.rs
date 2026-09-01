//! `SQLite` persistence adapter for Quadrant.

#![forbid(unsafe_code)]

mod connection;
mod mapping;
mod migrations;
mod store;

pub use store::SqliteStore;
