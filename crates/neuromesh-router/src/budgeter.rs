use neuromesh_core::OptimizationMode;

pub struct AdaptiveTokenBudgeter;

impl AdaptiveTokenBudgeter {
    pub fn calculate_budget(total_tokens: usize, mode: OptimizationMode) -> usize {
        match mode {
            OptimizationMode::MaxQuality => total_tokens, // No artificial token cap
            OptimizationMode::Balanced => {
                // Target 50-70% reduction -> budget is 35-45% of total
                (total_tokens as f32 * 0.40).max(2048.0) as usize
            }
            OptimizationMode::MaxSavings => {
                // Target aggressive reduction -> budget is 25% of total
                (total_tokens as f32 * 0.25).max(1024.0) as usize
            }
        }
    }
}
