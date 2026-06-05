use rand::seq::SliceRandom;
use wreq_util::{Emulation, EmulationOS, EmulationOption};

#[derive(Clone, Copy)]
pub struct Profile {
    pub label: &'static str,
    emu: Emulation,
    os: EmulationOS,
}

impl Profile {
    pub fn emulation(&self) -> EmulationOption {
        EmulationOption::builder()
            .emulation(self.emu)
            .emulation_os(self.os)
            .build()
    }
}

pub const PROFILES: &[Profile] = &[
    Profile { label: "Chrome 137 / Windows",  emu: Emulation::Chrome137, os: EmulationOS::Windows },
    Profile { label: "Chrome 137 / macOS",    emu: Emulation::Chrome137, os: EmulationOS::MacOS },
    Profile { label: "Chrome 136 / Linux",    emu: Emulation::Chrome136, os: EmulationOS::Linux },
    Profile { label: "Chrome 137 / Android",  emu: Emulation::Chrome137, os: EmulationOS::Android },
    Profile { label: "Firefox 139 / Windows", emu: Emulation::Firefox139, os: EmulationOS::Windows },
    Profile { label: "Firefox 136 / Linux",   emu: Emulation::Firefox136, os: EmulationOS::Linux },
    Profile { label: "Firefox / Android",     emu: Emulation::FirefoxAndroid135, os: EmulationOS::Android },
    Profile { label: "Edge 134 / Windows",    emu: Emulation::Edge134, os: EmulationOS::Windows },
    Profile { label: "Safari 18.3 / macOS",   emu: Emulation::Safari18_3, os: EmulationOS::MacOS },
    Profile { label: "Safari 18.5 / macOS",   emu: Emulation::Safari18_5, os: EmulationOS::MacOS },
    Profile { label: "Safari / iPhone",       emu: Emulation::SafariIos18_1_1, os: EmulationOS::IOS },
    Profile { label: "Safari / iPad",         emu: Emulation::SafariIPad18, os: EmulationOS::IOS },
];

impl Profile {
    pub fn random() -> Profile {
        let mut rng = rand::thread_rng();
        *PROFILES.choose(&mut rng).unwrap()
    }

    pub fn find(query: &str) -> Option<Profile> {
        let q = query.to_lowercase();
        PROFILES
            .iter()
            .find(|p| p.label.to_lowercase().contains(&q))
            .copied()
    }
}
