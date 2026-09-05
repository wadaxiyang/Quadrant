// SPDX-License-Identifier: GPL-3.0-only
//! Explicit process-owned diagnostic sink; never accepts task payloads.

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct AgentLog(Mutex<File>);
impl AgentLog {
    pub fn open(profile: &Path) -> io::Result<Self> {
        let directory = profile.join("logs");
        std::fs::create_dir_all(&directory)?;
        Ok(Self(Mutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(directory.join("quadrant-agent.log"))?,
        )))
    }

    pub fn event(&self, event: &'static str) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Ok(mut file) = self.0.lock() {
            let _ = writeln!(
                file,
                "timestamp={timestamp} process=quadrant-agent pid={} level=info event={event}",
                std::process::id()
            );
        }
    }
}
