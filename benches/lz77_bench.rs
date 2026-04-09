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

use criterion::BenchmarkId;
use glasik_core::tokenizer::sliding_v2::SlidingTokenizerV2;
use glasik_core::tokenizer::codon::FirstByteIndex;
use glasik_core::static_dict;

fn bench_sliding_components(c: &mut Criterion) {
    // Load real corpus chunk
    let data = b"There are many talented screenwriters out there, and finding the right one for your project can depend on several factors including budget, genre, experience level, and your specific needs. Here are some general avenues you might explore to find screenwriters. Online platforms such as Stage 32, Mandy, and ProductionHUB are platforms where you can find screenwriters looking for work. You can post a job listing or browse profiles. Freelance websites like Upwork, Freelancer, and Fiverr have sections for writers where you can find screenwriters. These platforms allow you to review portfolios and past work. Film school connections reaching out to film schools like UCLA, USC, NYU, or AFI can be a great way to find emerging talent. Many students and recent graduates are looking for opportunities to build their portfolios.".repeat(2);
    let buf = data.as_slice();

    // Warm window
    let static_entries = static_dict::load_static_dict();
    let mut slider = SlidingTokenizerV2::new_with_static(static_entries);
    for _ in 0..500 { slider.encode(buf); }

    let mut grp = c.benchmark_group("sliding_components");

    // Full encode
    grp.bench_function("full_encode", |b| b.iter(|| {
        black_box(slider.encode(black_box(buf)))
    }));

    // Just build()
    grp.bench_function("build_only", |b| b.iter(|| {
        black_box(glasik_core::tokenizer::dictionary::build(black_box(buf)))
    }));

    // Just update_window
    grp.bench_function("update_window", |b| b.iter(|| {
        let batch = glasik_core::tokenizer::dictionary::build(buf);
        black_box(slider.update_window_bench(black_box(&batch)))
    }));

    // Just FirstByteIndex::build
    let (_, entries) = slider.export_dict();
    let dict: Vec<_> = entries.into_iter().map(|(bytes,freq,saving)|
        glasik_core::tokenizer::dictionary::DictEntry{bytes,freq:freq as usize,saving:saving as usize}
    ).collect();
    grp.bench_function("index_build", |b| b.iter(|| {
        black_box(FirstByteIndex::build(black_box(&dict)))
    }));

    grp.finish();
}

criterion_group!(sliding_benches, bench_sliding_components);
criterion_main!(sliding_benches);
