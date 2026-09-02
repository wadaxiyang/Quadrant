//! Distribution-neutral update policy and presentation state.

/// Packaging channel that owns application replacement semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributionChannel {
    /// Source/development build with manual release discovery.
    Source,
    /// Portable archive distributed through GitHub Releases.
    Portable,
    /// Installation managed by an OS package/store mechanism.
    PackageManaged,
}

/// Static update capability projected into Settings/About.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateViewState {
    /// Canonical Cargo package version.
    pub current_version: String,
    /// Human-readable distribution/update ownership.
    pub description: String,
    /// Whether opening the project Releases page is useful for this channel.
    pub can_open_releases: bool,
}

impl UpdateViewState {
    /// Builds safe update behavior from package metadata.
    ///
    /// Unknown channel strings fail closed to a source/manual build. No mode
    /// attempts to replace the running executable in-process.
    #[must_use]
    pub fn from_build(current_version: impl Into<String>, channel: Option<&str>) -> Self {
        let channel = match channel {
            Some("portable" | "windows-portable") => DistributionChannel::Portable,
            Some("package-managed" | "linux-package" | "macos-bundle") => {
                DistributionChannel::PackageManaged
            }
            _ => DistributionChannel::Source,
        };
        let (description, can_open_releases) = match channel {
            DistributionChannel::Source => (
                "Source build · updates are installed manually".to_owned(),
                true,
            ),
            DistributionChannel::Portable => (
                "Portable build · download replacement releases manually".to_owned(),
                true,
            ),
            DistributionChannel::PackageManaged => (
                "Package-managed build · updates are owned by the installer or store".to_owned(),
                false,
            ),
        };
        Self {
            current_version: current_version.into(),
            description,
            can_open_releases,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateViewState;

    #[test]
    fn unknown_channels_fail_closed_to_manual_release_discovery() {
        let state = UpdateViewState::from_build("1.2.3", Some("unknown"));
        assert_eq!(state.current_version, "1.2.3");
        assert!(state.can_open_releases);
        assert!(state.description.contains("Source build"));
    }

    #[test]
    fn package_managed_builds_do_not_offer_in_process_replacement() {
        let state = UpdateViewState::from_build("1.2.3", Some("linux-package"));
        assert!(!state.can_open_releases);
        assert!(state.description.contains("installer or store"));
    }
}
