use neuromesh_core::SeedEngineId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RetrievalTier {
    L1,
    L2,
    L3,
}

impl RetrievalTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
        }
    }

    pub fn seed_engine(self, configured: SeedEngineId) -> SeedEngineId {
        match self {
            Self::L1 | Self::L2 => configured,
            Self::L3 => SeedEngineId::SemanticLite,
        }
    }

    pub fn graph_hops(self) -> u8 {
        match self {
            Self::L1 => 1,
            Self::L2 => 2,
            Self::L3 => 3,
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::L1, Self::L2, Self::L3]
    }
}
