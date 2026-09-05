// SPDX-License-Identifier: GPL-3.0-only
//! GUI transport only; no repository, application service, or desktop ownership.
#![forbid(unsafe_code)]

pub mod ipc;

/// Presentation selection parsed before IPC negotiation or Slint construction.
#[derive(Debug, PartialEq, Eq)]
pub struct GuiOptions {
    /// Surface announced in the IPC handshake.
    pub mode: quadrant_protocol::GuiLaunchMode,
    /// Agent-owned children must not resurrect their parent.
    pub agent_launched: bool,
}

impl GuiOptions {
    /// Parses GUI arguments without launching processes or creating UI.
    /// # Errors
    /// Rejects unknown arguments.
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> std::io::Result<Self> {
        let mut options = Self {
            mode: quadrant_protocol::GuiLaunchMode::Main,
            agent_launched: false,
        };
        for argument in arguments {
            match argument.as_str() {
                "--quick-add" => options.mode = quadrant_protocol::GuiLaunchMode::QuickAdd,
                "--agent-launched" => options.agent_launched = true,
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Unknown GUI argument",
                    ));
                }
            }
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_arguments_select_surface_and_preserve_parent_protection() {
        use quadrant_protocol::GuiLaunchMode::{Main, QuickAdd};
        for (arguments, mode, agent_launched) in [
            (vec![], Main, false),
            (vec!["--quick-add"], QuickAdd, false),
            (vec!["--agent-launched"], Main, true),
            (vec!["--quick-add", "--agent-launched"], QuickAdd, true),
            (vec!["--agent-launched", "--quick-add"], QuickAdd, true),
        ] {
            assert_eq!(
                GuiOptions::parse(arguments.into_iter().map(str::to_owned)).unwrap(),
                GuiOptions {
                    mode,
                    agent_launched
                }
            );
        }
        assert!(GuiOptions::parse(["--unknown".into()]).is_err());
    }
}
