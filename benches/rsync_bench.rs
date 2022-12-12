#[macro_use]
extern crate criterion;

#[path = "../src/crc.rs"]
#[allow(dead_code)]
#[allow(unused_imports)]
mod crc;

use crate::crc::Crc;
use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use fast_rsync::{apply_limited, diff, Signature, SignatureOptions};
use std::io;

fn random_block(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut v = vec![0; len];
    rand::thread_rng().fill_bytes(&mut v);
    v
}

fn crc_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("crc_update");
    for &len in &[1024, 4096] {
        let data = random_block(len);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("Crc::update", len), &data, |b, data| {
            b.iter(|| Crc::new().update(black_box(data)))
        });
        group.bench_with_input(
            BenchmarkId::new("Crc::basic_update", len),
            &data,
            |b, data| b.iter(|| Crc::new().basic_update(black_box(data))),
        );
    }
    group.finish();
}

criterion_group!(crc, crc_update);

fn calculate_signature(c: &mut Criterion) {
    let data = random_block(1 << 22);
    let mut group = c.benchmark_group("calculate_signature");
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.sample_size(20);
    group.bench_with_input(
        BenchmarkId::new("fast_rsync::Signature::calculate", data.len()),
        &data,
        |b, data| {
            b.iter(|| {
                Signature::calculate(
                    black_box(data),
                    SignatureOptions {
                        block_size: 4096,
                        crypto_hash_size: 8,
                    },
                )
                .into_serialized();
            })
        },
    );
    group.bench_with_input(
        BenchmarkId::new("librsync::whole::signature", data.len()),
        &data,
        |b, data| {
            b.iter(|| {
                let mut out = Vec::new();
                librsync::whole::signature_with_options(
                    &mut &data[..],
                    &mut out,
                    4096,
                    8,
                    librsync::SignatureType::MD4,
                )
                .unwrap();
                out
            })
        },
    );
    group.finish();
}

fn bench_diff(
    c: &mut Criterion,
    name: &str,
    data: &[u8],
    new_data: &Vec<u8>,
    allow_librsync: bool,
) {
    let signature = Signature::calculate(
        data,
        SignatureOptions {
            block_size: 4096,
            crypto_hash_size: 8,
        },
    )
    .into_serialized();
    let mut group = c.benchmark_group(name);
    group.sample_size(15);
    group.bench_with_input(
        BenchmarkId::new("fast_rsync::diff", new_data.len()),
        new_data,
        |b, new_data| {
            b.iter(|| {
                let sig = Signature::deserialize(signature.clone()).unwrap();
                let sig = sig.index();
                let mut out = Vec::new();
                diff(&sig, black_box(new_data), &mut out).unwrap();
                out
            })
        },
    );
    if allow_librsync {
        group.bench_with_input(
            BenchmarkId::new("librsync::whole::delta", new_data.len()),
            new_data,
            |b, new_data| {
                b.iter(|| {
                    let mut out = Vec::new();
                    librsync::whole::delta(
                        &mut black_box(&new_data[..]),
                        &mut &signature[..],
                        &mut out,
                    )
                    .unwrap();
                    out
                })
            },
        );
    }
    group.finish();
}

fn calculate_diff(c: &mut Criterion) {
    let data = random_block(1 << 22);
    let mut new_data = data.clone();
    new_data[1000000..1065536].copy_from_slice(&random_block(65536));
    bench_diff(c, "diff (64KB edit)", &data, &new_data, true);
    bench_diff(c, "diff (random)", &data, &random_block(1 << 22), true);
    bench_diff(
        c,
        "diff (pathological)",
        &vec![0; 1 << 14],
        &vec![128; 1 << 14],
        true,
    );
    bench_diff(
        c,
        "diff (pathological)",
        &vec![0; 1 << 22],
        &vec![128; 1 << 22],
        false,
    );
}

fn apply_delta(c: &mut Criterion) {
    let data = random_block(1 << 22);
    let mut new_data = data.clone();
    new_data[1000000..1065536].copy_from_slice(&random_block(65536));
    let mut delta = Vec::new();
    diff(
        &Signature::calculate(
            &data,
            SignatureOptions {
                block_size: 4096,
                crypto_hash_size: 8,
            },
        )
        .index(),
        &new_data,
        &mut delta,
    )
    .unwrap();
    let mut group = c.benchmark_group("apply");
    group.bench_with_input(
        BenchmarkId::new("fast_rsync::apply", new_data.len()),
        &delta,
        |b, delta| {
            b.iter(|| {
                let mut out = Vec::new();
                apply_limited(&data, delta, &mut out, 1 << 22).unwrap();
                out
            })
        },
    );
    group.bench_with_input(
        BenchmarkId::new("librsync::whole::patch", new_data.len()),
        &delta,
        |b, delta| {
            b.iter(|| {
                let mut out = Vec::new();
                librsync::whole::patch(&mut io::Cursor::new(&data[..]), &mut &delta[..], &mut out)
                    .unwrap();
                out
            })
        },
    );
    group.finish();
}

criterion_group!(rsync, calculate_signature, calculate_diff, apply_delta);

criterion_main!(crc, rsync);
