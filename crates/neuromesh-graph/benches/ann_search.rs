use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use neuromesh_core::NodeId;
use neuromesh_graph::embeddings::{dot_f32_f32, EmbeddingIndex};

fn random_unit_matrix(n: usize, dim: usize, seed: u64) -> Vec<f32> {
    let mut out = Vec::with_capacity(n * dim);
    for i in 0..n {
        let mut v = vec![0.0f32; dim];
        let mut s = seed.wrapping_add(i as u64);
        for x in &mut v {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            *x = ((s >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for x in &mut v {
                *x /= norm;
            }
        }
        out.extend_from_slice(&v);
    }
    out
}

fn build_index(n: usize, dim: usize) -> (EmbeddingIndex, Vec<f32>) {
    let vectors = random_unit_matrix(n, dim, 42);
    let node_ids: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("sym:{i}"))).collect();
    let index = EmbeddingIndex {
        model_id: "bench".into(),
        dim,
        node_ids,
        vectors,
        ..Default::default()
    };
    let query = random_unit_matrix(1, dim, 999)[..dim].to_vec();
    (index, query)
}

fn bench_ann(c: &mut Criterion) {
    let dim = 384;
    let mut group = c.benchmark_group("ann_search");

    for n in [1_000usize, 8_000] {
        let (index, query) = build_index(n, dim);
        group.bench_with_input(BenchmarkId::new("full_scan", n), &n, |b, _| {
            b.iter(|| {
                black_box(index.ann_search(black_box(&query), 16, 0.45));
            });
        });

        let pool: Vec<usize> = (0..400.min(n)).collect();
        group.bench_with_input(BenchmarkId::new("subset_400", n), &n, |b, _| {
            b.iter(|| {
                black_box(index.ann_search_subset(black_box(&query), black_box(&pool), 16, 0.45));
            });
        });
    }

    group.finish();
}

fn bench_dot(c: &mut Criterion) {
    let dim = 384;
    let a = random_unit_matrix(1, dim, 1);
    let b = random_unit_matrix(1, dim, 2);
    c.bench_function("dot_f32_f32_384", |bench| {
        bench.iter(|| black_box(dot_f32_f32(black_box(&a), black_box(&b))));
    });
}

criterion_group!(benches, bench_ann, bench_dot);
criterion_main!(benches);
