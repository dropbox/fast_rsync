# fast\_rsync

[![crates.io](https://meritbadge.herokuapp.com/fast_rsync)](https://crates.io/crates/fast_rsync)
![Build Status](https://github.com/dropbox/fast_rsync/workflows/Rust/badge.svg)

[Documentation](https://docs.rs/fast_rsync)

An faster implementation of [librsync](https://github.com/librsync/librsync) in
pure Rust, using SIMD operations where available. Note that only the legacy MD4
format is supported, not BLAKE2.

Rust nightly is currently required because of
[packed\_simd](https://github.com/rust-lang/packed_simd).

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
   _probably_ (*) equal to `foo_A`.

(*) Note the caveat. `fast_rsync` signatures use the insecure MD4 algorithm.
Therefore, you should not trust that `diff` will produce a correct delta. You
must always verify the integrity of the output of `apply` using some other
mechanism, such as a cryptographic hash function like SHA-256.

## Benchmarks
These were taken on a noisy laptop with a `Intel(R) Core(TM) i7-6820HQ CPU @ 2.70GHz`.

```
calculate_signature/fast_rsync::Signature::calculate/4194304
                        time:   [1.0453 ms 1.0615 ms 1.0747 ms]
                        thrpt:  [3.6348 GiB/s 3.6801 GiB/s 3.7371 GiB/s]
calculate_signature/librsync::whole::signature/4194304
                        time:   [6.4568 ms 6.5294 ms 6.6208 ms]
                        thrpt:  [604.16 MiB/s 612.61 MiB/s 619.50 MiB/s]
diff (64KB edit)/fast_rsync::diff/4194304
                        time:   [7.8734 ms 7.9300 ms 7.9925 ms]
diff (64KB edit)/librsync::whole::delta/4194304
                        time:   [7.5317 ms 7.6264 ms 7.7118 ms]
diff (random)/fast_rsync::diff/4194304
                        time:   [55.319 ms 57.576 ms 60.292 ms]
diff (random)/librsync::whole::delta/4194304
                        time:   [44.500 ms 44.791 ms 45.220 ms]
diff (pathological)/fast_rsync::diff/16384
                        time:   [6.6302 ms 6.6615 ms 6.7008 ms]
diff (pathological)/librsync::whole::delta/16384
                        time:   [51.178 ms 51.591 ms 51.985 ms]
diff (pathological)/fast_rsync::diff/4194304
                        time:   [41.568 ms 41.814 ms 42.090 ms]
apply/fast_rsync::apply/4194304
                        time:   [324.01 us 327.43 us 331.64 us]
apply/librsync::whole::patch/4194304
                        time:   [426.44 us 428.73 us 431.16 us]
```

## Contributing
Pull requests are welcome! We ask that you agree to [Dropbox's Contributor
License Agreement](https://opensource.dropbox.com/cla/) for your changes to be
merged.

## License
This project is licensed under [the Apache-2.0
license](http://www.apache.org/licenses/LICENSE-2.0).

Copyright (c) 2019 Dropbox, Inc.  
Copyright (c) 2016 bacher09, Artyom Pavlov (RustCrypto/hashes/MD4).
