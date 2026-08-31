use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinuxBackend {
    Wayland,
    X11,
    Headless,
}

impl LinuxBackend {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Wayland => "wayland",
            Self::X11 => "x11",
            Self::Headless => "headless",
        }
    }
}

impl FromStr for LinuxBackend {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "wayland" => Ok(Self::Wayland),
            "x11" => Ok(Self::X11),
            "headless" => Ok(Self::Headless),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_backend_ids_round_trip() {
        for backend in [
            LinuxBackend::Wayland,
            LinuxBackend::X11,
            LinuxBackend::Headless,
        ] {
            assert_eq!(backend.id().parse(), Ok(backend));
        }
    }
}
