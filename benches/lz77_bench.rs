use criterion::{criterion_group, criterion_main, Criterion, black_box};
use glasik_core::tokenizer::lz77_gn::GNPrefixTokenizer;
use glasik_core::tokenizer::dictionary::DictEntry;
use glasik_core::tokenizer::sliding_v2::SlidingTokenizerV2;
use glasik_core::static_dict;

fn make_entries(count: usize) -> Vec<DictEntry> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let s = format!("user: assistant: hello world question answer {i:04} token pattern");
        out.push(DictEntry { bytes: s.into_bytes(), freq: 1, saving: 1 });
    }
    out
}

fn real_entries() -> Vec<DictEntry> {
    let entries = static_dict::load_static_dict();
    let mut slider = SlidingTokenizerV2::new_with_static(entries);
    // warm with synthetic data
    let warm = b"user: hello assistant: how can I help you today with your question about compression and sliding windows and JWT authentication bearer tokens middleware express API validation error handling database optimization".repeat(10);
    for chunk in warm.chunks(1200) {
        slider.encode(chunk);
    }
    let (_, dict_entries) = slider.export_dict();
    dict_entries.into_iter().map(|(bytes, freq, saving)| DictEntry { bytes, freq: freq as usize, saving: saving as usize }).collect()
}

fn bench_prefix4_synthetic(c: &mut Criterion) {
    let entries = make_entries(500);
    let mut tok = GNPrefixTokenizer::<4>::new();
    tok.seed_from_vocab(&entries);
    let buf: Vec<u8> = b"user: assistant: hello world question answer 0001 token pattern ".repeat(20)[..1200].to_vec();

    c.bench_function("lz77_gn prefix4 synthetic 500 patterns", |b| {
        b.iter(|| {
            let out = tok.tokenize_to_gn_bytes(black_box(&buf), true);
            black_box(out.len());
        })
    });
}

fn bench_prefix4_real(c: &mut Criterion) {
    let entries = real_entries();
    let mut tok = GNPrefixTokenizer::<4>::new();
    tok.seed_from_vocab(&entries);
    let buf: Vec<u8> = b"user: hello assistant: how can I help you today with your question about JWT authentication bearer tokens middleware express API validation error handling database optimization PostgreSQL EXPLAIN ANALYZE composite index performance".repeat(6)[..1200].to_vec();

    c.bench_function("lz77_gn prefix4 real vocab", |b| {
        b.iter(|| {
            let out = tok.tokenize_to_gn_bytes(black_box(&buf), true);
            black_box(out.len());
        })
    });
}

fn bench_prefix3_real(c: &mut Criterion) {
    let entries = real_entries();
    let mut tok = GNPrefixTokenizer::<3>::new();
    tok.seed_from_vocab(&entries);
    let buf: Vec<u8> = b"user: hello assistant: how can I help you today with your question about JWT authentication bearer tokens middleware express API validation error handling database optimization PostgreSQL EXPLAIN ANALYZE composite index performance".repeat(6)[..1200].to_vec();

    c.bench_function("lz77_gn prefix3 real vocab", |b| {
        b.iter(|| {
            let out = tok.tokenize_to_gn_bytes(black_box(&buf), true);
            black_box(out.len());
        })
    });
}

criterion_group!(benches, bench_prefix4_synthetic, bench_prefix4_real, bench_prefix3_real);
criterion_main!(benches);
