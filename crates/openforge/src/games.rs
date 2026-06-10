use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Game {
    Minecraft,
    WowRetail,
    WowClassic,
    Sims4,
    ArkSurvivalEvolved,
    Other(u64),
}

impl Game {
    pub fn id(self) -> u64 {
        match self {
            Game::Minecraft => 432,
            Game::WowRetail => 1,
            Game::WowClassic => 1,
            Game::Sims4 => 78,
            Game::ArkSurvivalEvolved => 0,
            Game::Other(x) => x,
        }
    }

    pub fn from_id(id: u64) -> Self {
        match id {
            432 => Game::Minecraft,
            1   => Game::WowRetail,
            78  => Game::Sims4,
            other => Game::Other(other),
        }
    }

    pub fn from_slug(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "minecraft" | "mc" => Game::Minecraft,
            "wow" | "world-of-warcraft" => Game::WowRetail,
            "wow-classic" => Game::WowClassic,
            "sims4" | "the-sims-4" => Game::Sims4,
            "ark" | "ark-survival-evolved" => Game::ArkSurvivalEvolved,
            _ => return None,
        })
    }
}

pub trait GameAdapter: Send + Sync {
    fn game(&self) -> Game;
    fn install_dir(&self, instance: &str) -> PathBuf;
    fn loader_required(&self) -> bool { false }
}

pub struct MinecraftAdapter;
impl GameAdapter for MinecraftAdapter {
    fn game(&self) -> Game { Game::Minecraft }
    fn install_dir(&self, instance: &str) -> PathBuf {
        crate::core::paths::instances_dir().join(instance).join("mods")
    }
    fn loader_required(&self) -> bool { true }
}

pub struct WowAdapter { pub retail: bool, pub wow_root: PathBuf }
impl GameAdapter for WowAdapter {
    fn game(&self) -> Game { if self.retail { Game::WowRetail } else { Game::WowClassic } }
    fn install_dir(&self, _instance: &str) -> PathBuf {
        let branch = if self.retail { "_retail_" } else { "_classic_" };
        self.wow_root.join(branch).join("Interface").join("AddOns")
    }
}

pub struct Sims4Adapter;
impl GameAdapter for Sims4Adapter {
    fn game(&self) -> Game { Game::Sims4 }
    fn install_dir(&self, _instance: &str) -> PathBuf {
        dirs::document_dir().unwrap_or_default()
            .join("Electronic Arts").join("The Sims 4").join("Mods")
    }
}

pub struct ArkAdapter { pub ark_root: PathBuf }
impl GameAdapter for ArkAdapter {
    fn game(&self) -> Game { Game::ArkSurvivalEvolved }
    fn install_dir(&self, _instance: &str) -> PathBuf {
        self.ark_root.join("ShooterGame").join("Content").join("Mods")
    }
}
