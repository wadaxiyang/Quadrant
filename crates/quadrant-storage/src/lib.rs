//! Storage adapter boundary for the future `rusqlite` implementation.

#![forbid(unsafe_code)]

/// Marker for the concrete storage adapter assembled by the app crate.
#[derive(Debug, Default)]
pub struct StorageAdapter;
