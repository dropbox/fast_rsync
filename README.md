# fast\_rsync

[![Crates.io](https://img.shields.io/crates/v/fast_rsync.svg)](https://crates.io/crates/fast_rsync)
[![Build Status](https://github.com/dropbox/fast_rsync/workflows/Rust/badge.svg)](https://github.com/dropbox/fast_rsync/actions)

[Documentation](https://docs.rs/fast_rsync)

A faster implementation of [librsync](https://github.com/librsync/librsync) in
pure Rust, using SIMD operations where available. Note that only the legacy MD4
format is supported, not BLAKE2.

SIMD is currently supported on x86, x86-64, and aarch64 targets.

## The rsync algorithm
This crate offers three major APIs:

1. `Signature::calculate`, which takes a block of data and returns a
   "signature" of that data which is much smaller than the original data.
2. `diff`, which takes a signature for some block A, and a block of data B, and
   returns a delta between block A and block B. If A and B are "similar", then
   the delta is usually much smaller than block B.
3. `apply`, which takes a block A and a delta (as constructed by `diff`), and
   (usually) returns the block B.

These functions can be used to implement an protocol for efficiently
transferring data over a network. Suppose hosts A and B have similar versions
of some file `foo`, and host B would like to acquire A's copy.
1. Host B calculates the `Signature` of `foo_B` and sends it to A. This is
   cheap because the signature can be 1000X smaller than `foo_B` itself. (The
   precise factor is configurable and creates a tradeoff between signature size
   and usefulness. A larger signature enables the creation of smaller and more
   precise deltas.)
2. Host A calculates a `diff` from B's signature and `foo_A`, and sends it to
   `B`.
3. Host B attempts to `apply` the delta to `foo_B`. The resulting data is
   _probably_ (\*) equal to `foo_A`.

(\*) Note the caveat. `fast_rsync` signatures use the insecure MD4 algorithm.
Therefore, you should not trust that `diff` will produce a correct delta. You
must always verify the integrity of the output of `apply` using some other
mechanism, such as a cryptographic hash function like SHA-256.

## Benchmarks
These were taken on a noisy laptop with a `Intel(R) Core(TM) i7-6820HQ CPU @
2.70GHz`. The source code is available in `benches/rsync_bench.rs`.

### Signature computation
```
calculate_signature/fast_rsync::Signature::calculate/4194304
                        time:   [1.0639 ms 1.0696 ms 1.0775 ms]
                        thrpt:  [3.6253 GiB/s 3.6519 GiB/s 3.6716 GiB/s]
calculate_signature/librsync::whole::signature/4194304
                        time:   [5.8013 ms 5.8521 ms 5.9235 ms]
                        thrpt:  [675.28 MiB/s 683.51 MiB/s 689.50 MiB/s]
```

`fast_rsync` is substantially faster than `librsync` at calculating signatures,
thanks to SIMD optimizations. The benchmark processor has AVX2 and sees a 6X
speedup. Processors with only SSE2 (or with less fully-featured AVX) see a
smaller speedup, about 3-4X.

Note that `fast_rsync` will detect available vector extensions at runtime and
use them as appropriate; `-C target-cpu` is not required.

### Computing deltas
```
diff (64KB edit)/fast_rsync::diff/4194304
                        time:   [6.8681 ms 7.0596 ms 7.1953 ms]
diff (64KB edit)/librsync::whole::delta/4194304
                        time:   [7.4044 ms 7.4649 ms 7.5222 ms]
```

When comparing similar files, `fast_rsync` is mostly bound by the speed of
single-block MD4 hashing, so it is not much faster than `librsync`.

```
diff (random)/fast_rsync::diff/4194304
                        time:   [37.779 ms 38.317 ms 38.607 ms]
diff (random)/librsync::whole::delta/4194304
                        time:   [41.983 ms 42.758 ms 43.259 ms]
```

When comparing completely different files, `fast_rsync` is mostly bound by the
speed of hashmap lookups. Here, `fast_rsync` enjoys a slight advantage because
of Rust's fast built-in `HashMap` implementation.

```
diff (pathological)/fast_rsync::diff/16384
                        time:   [6.0792 ms 6.2550 ms 6.3666 ms]
diff (pathological)/librsync::whole::delta/16384
                        time:   [50.082 ms 50.185 ms 50.376 ms]
diff (pathological)/fast_rsync::diff/4194304
                        time:   [32.690 ms 32.986 ms 33.171 ms]
```

`fast_rsync` is able to detect pathological cases that involve many checksum
collisions. Note that the 4MB version of the benchmark is prohibitively slow
for `librsync` and so its result is not listed.

### Applying deltas
```
apply/fast_rsync::apply/4194304
                        time:   [276.17 us 284.20 us 293.37 us]
apply/librsync::whole::patch/4194304
                        time:   [394.21 us 400.30 us 408.79 us]
```

Applying deltas is quite straightforward and in any case is unlikely to be a
bottleneck, but `fast_rsync`'s implementation, which is specialized for
in-memory buffers, enjoys a mild speedup.

## Contributing
Pull requests are welcome! We ask that you agree to [Dropbox's Contributor
License Agreement](https://opensource.dropbox.com/cla/) for your changes to be
merged.

## License
This project is licensed under [the Apache-2.0
license](http://www.apache.org/licenses/LICENSE-2.0).

Copyright (c) 2019 Dropbox, Inc.  
Copyright (c) 2016 bacher09, Artyom Pavlov (RustCrypto/hashes/MD4).
