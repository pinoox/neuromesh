use crate::retrieval::tier::RetrievalTier;
use serde::{Deserialize, Serialize};

/// Configurable per-tier latency, token, and compute budgets (defaults — not hard-coded in logic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierBudget {
    pub latency_ms: u64,
    pub selected_tokens: usize,
    pub max_gap_rounds: u8,
}

impl Default for TierBudget {
    fn default() -> Self {
        Self {
            latency_ms: 30,
            selected_tokens: 4_000,
            max_gap_rounds: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalBudget {
    pub l1: TierBudget,
    pub l2: TierBudget,
    pub l3: TierBudget,
}

impl Default for RetrievalBudget {
    fn default() -> Self {
        Self {
            l1: TierBudget {
                latency_ms: 30,
                selected_tokens: 2_000,
                max_gap_rounds: 0,
            },
            l2: TierBudget {
                latency_ms: 100,
                selected_tokens: 8_000,
                max_gap_rounds: 2,
            },
            l3: TierBudget {
                latency_ms: 200,
                selected_tokens: 16_000,
                max_gap_rounds: 2,
            },
        }
    }
}

impl RetrievalBudget {
    pub fn for_tier(&self, tier: RetrievalTier) -> &TierBudget {
        match tier {
            RetrievalTier::L1 => &self.l1,
            RetrievalTier::L2 => &self.l2,
            RetrievalTier::L3 => &self.l3,
        }
    }
}
