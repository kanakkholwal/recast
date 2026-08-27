use core::fmt;

/// A capability the OS gates behind user consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum PermissionKind {
    /// Reading the contents of displays or windows.
    Screen,
    /// Reading a camera.
    Camera,
    /// Reading a microphone.
    Microphone,
}

impl fmt::Display for PermissionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Screen => "screen",
            Self::Camera => "camera",
            Self::Microphone => "microphone",
        })
    }
}

/// Whether a capability may be used, and if not, whether asking would help.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum Permission {
    /// Granted; capture will work.
    Granted,
    /// Refused by the user. Asking again does nothing until they change it in
    /// system settings.
    Denied,
    /// Never asked. A prompt will appear on the first request.
    NotDetermined,
    /// Blocked by policy or parental controls, not by the user.
    Restricted,
    /// The platform does not gate this capability at all.
    NotRequired,
}

impl Permission {
    /// Whether capture can proceed right now.
    #[must_use]
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Granted | Self::NotRequired)
    }

    /// Whether prompting the user could change the answer.
    ///
    /// False for `Denied`: macOS and Wayland both silently no-op a second
    /// request, so a caller that keeps asking shows the user nothing and looks
    /// broken. Send them to system settings instead.
    #[must_use]
    pub const fn is_requestable(self) -> bool {
        matches!(self, Self::NotDetermined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ungated_platform_counts_as_usable() {
        assert!(Permission::NotRequired.is_usable());
        assert!(Permission::Granted.is_usable());
    }

    #[test]
    fn only_an_unasked_permission_is_worth_prompting_for() {
        assert!(Permission::NotDetermined.is_requestable());
        assert!(!Permission::Denied.is_requestable());
        assert!(!Permission::Restricted.is_requestable());
        assert!(!Permission::Granted.is_requestable());
    }

    #[test]
    fn a_restricted_permission_is_neither_usable_nor_requestable() {
        assert!(!Permission::Restricted.is_usable());
        assert!(!Permission::Restricted.is_requestable());
    }
}
