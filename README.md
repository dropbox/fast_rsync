# fast\_rsync

[![crates.io](https://meritbadge.herokuapp.com/fast_rsync)](https://crates.io/crates/fast_rsync)
[![Build Status](https://travis-ci.org/dropbox/fast_rsync.svg?branch=master)](https://travis-ci.org/dropbox/fast_rsync)

[Documentation](https://docs.rs/fast_rsync)

An faster implementation of [librsync](https://github.com/librsync/librsync) in
pure Rust, using SIMD operations where available.

Rust nightly is currently required because of
[packed\_simd](https://github.com/rust-lang/packed_simd).

## Contributing
Pull requests are welcome! We ask that you agree to [Dropbox's Contributor
License Agreement](https://opensource.dropbox.com/cla/) for your changes to be
merged.

## License
This project is licensed under [the Apache-2.0
license](http://www.apache.org/licenses/LICENSE-2.0).

Copyright (c) 2019 Dropbox, Inc.  
Copyright (c) 2016 bacher09, Artyom Pavlov (RustCrypto/hashes/MD4).
