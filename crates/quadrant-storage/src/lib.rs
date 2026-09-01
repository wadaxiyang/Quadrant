//! `SQLite` persistence adapter for Quadrant.

#![forbid(unsafe_code)]

mod connection;
mod focus;
mod mapping;
mod migrations;
mod review;
mod store;

pub use store::SqliteStore;
