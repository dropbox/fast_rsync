# fast\_rsync

[![crates.io](https://meritbadge.herokuapp.com/fast_rsync)](https://crates.io/crates/fast_rsync)
![Build Status](https://github.com/dropbox/fast_rsync/workflows/Rust/badge.svg)

[Documentation](https://docs.rs/fast_rsync)

An faster implementation of [librsync](https://github.com/librsync/librsync) in
pure Rust, using SIMD operations where available.

Rust nightly is currently required because of
[packed\_simd](https://github.com/rust-lang/packed_simd).

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
