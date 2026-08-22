use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChromosome {
    pub fold_threshold_lines: usize,
    pub docstring_retention: f32,
    pub type_signature_depth: usize,
    pub interface_boost: f32,
    pub fitness_score: f32,
}

impl Default for ContextChromosome {
    fn default() -> Self {
        Self {
            fold_threshold_lines: 4,
            docstring_retention: 0.8,
            type_signature_depth: 2,
            interface_boost: 1.5,
            fitness_score: 0.0,
        }
    }
}

impl ContextChromosome {
    pub fn new_random(seed: u64) -> Self {
        let pseudo_rand1 = ((seed.wrapping_mul(1103515245) + 12345) % 100) as f32 / 100.0;
        let pseudo_rand2 = ((seed.wrapping_mul(1664525) + 1013904223) % 100) as f32 / 100.0;
        let fold_lines = 3 + ((seed % 7) as usize);

        Self {
            fold_threshold_lines: fold_lines,
            docstring_retention: 0.5 + (pseudo_rand1 * 0.5),
            type_signature_depth: 1 + ((seed % 3) as usize),
            interface_boost: 1.0 + pseudo_rand2,
            fitness_score: 0.0,
        }
    }

    pub fn mutate(&mut self, mutation_rate: f32, seed: u64) {
        if (seed % 100) as f32 / 100.0 < mutation_rate {
            let delta = if seed.is_multiple_of(2) { 1 } else { -1 };
            self.fold_threshold_lines =
                (self.fold_threshold_lines as isize + delta).clamp(2, 15) as usize;
            self.docstring_retention =
                (self.docstring_retention + (delta as f32 * 0.05)).clamp(0.1, 1.0);
            self.interface_boost = (self.interface_boost + (delta as f32 * 0.1)).clamp(0.5, 3.0);
        }
    }

    pub fn crossover(&self, other: &Self) -> Self {
        Self {
            fold_threshold_lines: (self.fold_threshold_lines + other.fold_threshold_lines) / 2,
            docstring_retention: (self.docstring_retention + other.docstring_retention) / 2.0,
            type_signature_depth: self.type_signature_depth.max(other.type_signature_depth),
            interface_boost: (self.interface_boost + other.interface_boost) / 2.0,
            fitness_score: 0.0,
        }
    }
}

pub struct GeneticContextOptimizer {
    pub population: Vec<ContextChromosome>,
    pub generation: usize,
    pub best_chromosome: ContextChromosome,
}

impl Default for GeneticContextOptimizer {
    fn default() -> Self {
        Self::new(10)
    }
}

impl GeneticContextOptimizer {
    pub fn new(population_size: usize) -> Self {
        let mut population = Vec::with_capacity(population_size);
        for i in 0..population_size {
            population.push(ContextChromosome::new_random((i as u64) + 42));
        }

        let best = population[0].clone();
        Self {
            population,
            generation: 0,
            best_chromosome: best,
        }
    }

    pub fn evaluate_fitness(
        &mut self,
        index: usize,
        original_tokens: usize,
        skeleton_tokens: usize,
        is_syntactically_sound: bool,
        type_safety_score: f32,
    ) -> f32 {
        if original_tokens == 0 {
            return 0.0;
        }

        let compression_ratio =
            (original_tokens.saturating_sub(skeleton_tokens)) as f32 / original_tokens as f32;
        let syntax_factor = if is_syntactically_sound { 1.0 } else { 0.0 };
        let type_factor = type_safety_score.clamp(0.0, 1.0);

        // Multi-objective fitness function
        let score = (0.50 * compression_ratio) + (0.35 * syntax_factor) + (0.15 * type_factor);

        if index < self.population.len() {
            self.population[index].fitness_score = score;
            if score > self.best_chromosome.fitness_score {
                self.best_chromosome = self.population[index].clone();
            }
        }

        score
    }

    pub fn evolve_generation(&mut self) {
        self.population.sort_by(|a, b| {
            b.fitness_score
                .partial_cmp(&a.fitness_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.generation += 1;

        let mut next_gen = Vec::with_capacity(self.population.len());
        // Elitism: keep top 2
        if let Some(top1) = self.population.first() {
            next_gen.push(top1.clone());
        }
        if self.population.len() > 1 {
            next_gen.push(self.population[1].clone());
        }

        // Breed remaining
        while next_gen.len() < self.population.len() {
            let parent_a = &self.population[next_gen.len() % self.population.len().min(4)];
            let parent_b = &self.population[(next_gen.len() + 1) % self.population.len().min(4)];
            let mut child = parent_a.crossover(parent_b);
            child.mutate(0.20, (self.generation as u64) + (next_gen.len() as u64));
            next_gen.push(child);
        }

        self.population = next_gen;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genetic_optimizer_evolution() {
        let mut optimizer = GeneticContextOptimizer::new(6);
        for i in 0..6 {
            optimizer.evaluate_fitness(i, 1000, 100, true, 1.0);
        }
        assert!(optimizer.best_chromosome.fitness_score > 0.8);
        optimizer.evolve_generation();
        assert_eq!(optimizer.generation, 1);
    }
}
