//! A SIMD-ized implementation of MD4 designed to hash many blocks in parallel.
//! The base implementation is derived from https://github.com/RustCrypto/hashes/tree/master/md4.
#![allow(clippy::ptr_offset_with_cast)]

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};

mod simd_transpose;

pub const MD4_SIZE: usize = 16;

// initial values for Md4State
const S: [u32; 4] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476];

macro_rules! md4 {
    (($($prefix:tt)*), $u32:ty, $add:path, $($s:path)?) => {
        #[derive(Copy, Clone)]
        struct Md4State {
            s: [$u32; 4],
        }

        #[allow(unused_parens)]
        impl Md4State {
            $($prefix)*
            fn process_block(&mut self, data: &[$u32; 16]) {
                #[inline(always)]
                fn f(x: $u32, y: $u32, z: $u32) -> $u32 {
                    (x & y) | (!x & z)
                }

                #[inline(always)]
                fn g(x: $u32, y: $u32, z: $u32) -> $u32 {
                    (x & y) | (x & z) | (y & z)
                }

                #[inline(always)]
                fn h(x: $u32, y: $u32, z: $u32) -> $u32 {
                    x ^ y ^ z
                }

                #[inline(always)]
                fn op1(a: $u32, b: $u32, c: $u32, d: $u32, k: $u32, s: u32) -> $u32 {
                    $add($add(a, f(b, c, d)), k).rotate_left($($s)?(s))
                }

                #[inline(always)]
                fn op2(a: $u32, b: $u32, c: $u32, d: $u32, k: $u32, s: u32) -> $u32 {
                    $add($add($add(a, g(b, c, d)), k), 0x5A82_7999).rotate_left($($s)?(s))
                }

                #[inline(always)]
                fn op3(a: $u32, b: $u32, c: $u32, d: $u32, k: $u32, s: u32) -> $u32 {
                    $add($add($add(a, h(b, c, d)), k), 0x6ED9_EBA1).rotate_left($($s)?(s))
                }

                let mut a = self.s[0];
                let mut b = self.s[1];
                let mut c = self.s[2];
                let mut d = self.s[3];

                // round 1
                for &i in &[0, 4, 8, 12] {
                    a = op1(a, b, c, d, data[i], 3);
                    d = op1(d, a, b, c, data[i + 1], 7);
                    c = op1(c, d, a, b, data[i + 2], 11);
                    b = op1(b, c, d, a, data[i + 3], 19);
                }

                // round 2
                for i in 0..4 {
                    a = op2(a, b, c, d, data[i], 3);
                    d = op2(d, a, b, c, data[i + 4], 5);
                    c = op2(c, d, a, b, data[i + 8], 9);
                    b = op2(b, c, d, a, data[i + 12], 13);
                }

                // round 3
                for &i in &[0, 2, 1, 3] {
                    a = op3(a, b, c, d, data[i], 3);
                    d = op3(d, a, b, c, data[i + 8], 9);
                    c = op3(c, d, a, b, data[i + 4], 11);
                    b = op3(b, c, d, a, data[i + 12], 15);
                }

                self.s[0] = $add(self.s[0], a);
                self.s[1] = $add(self.s[1], b);
                self.s[2] = $add(self.s[2], c);
                self.s[3] = $add(self.s[3], d);
            }
        }
    };
}

md4!((), u32, u32::wrapping_add,);

fn load_block(input: &[u8; 64]) -> [u32; 16] {
    macro_rules! split {
        ($($name: ident $(. $dummy:tt)*)*) => ({
            let ($($name),*) = array_refs![input, $(4 $($dummy)*),*];
            [$(u32::from_le_bytes(*$name)),*]
        });
    }
    split!(x0 x1 x2 x3 x4 x5 x6 x7 x8 x9 x10 x11 x12 x13 x14 x15)
}

pub fn md4(data: &[u8]) -> [u8; 16] {
    let mut state = Md4State { s: S };
    let mut chunks = data.chunks_exact(64);
    for block in &mut chunks {
        state.process_block(&load_block(array_ref![block, 0, 64]));
    }
    let remainder = chunks.remainder();
    let mut last_blocks = [0; 128];
    last_blocks[..remainder.len()].copy_from_slice(remainder);
    last_blocks[remainder.len()] = 0x80;
    let end = if remainder.len() >= 56 { 128 } else { 64 };
    *array_mut_ref![&mut last_blocks, end - 8, 8] = (data.len() as u64 * 8).to_le_bytes();
    let (last_block_0, last_block_1) = array_refs![&last_blocks, 64, 64];
    state.process_block(&load_block(last_block_0));
    if end == 128 {
        state.process_block(&load_block(last_block_1));
    }
    let mut digest = [0; 16];
    let (a, b, c, d) = mut_array_refs!(&mut digest, 4, 4, 4, 4);
    *a = state.s[0].to_le_bytes();
    *b = state.s[1].to_le_bytes();
    *c = state.s[2].to_le_bytes();
    *d = state.s[3].to_le_bytes();
    digest
}

mod simd {
    pub const MAX_LANES: usize = 8;

    pub struct Md4xN {
        lanes: usize,
        fun: fn(&[&[u8]]) -> [[u8; 16]; MAX_LANES],
    }

    macro_rules! n_lanes {
        ($u32xN:path, $load:path, $feature:tt) => (
            use crate::md4::S;
            use crate::md4::simd::{Md4xN, MAX_LANES};
            use arrayref::{array_ref, mut_array_refs};
            use std::ops::Add;

            #[allow(non_camel_case_types)]
            type u32xN = $u32xN;
            pub const LANES: usize = u32xN::lanes();

            md4!((#[target_feature(enable = $feature)] unsafe), u32xN, u32xN::add, u32xN::splat);

            /// Compute the MD4 sum of multiple equally-sized blocks of data.
            /// Unsafety: This function requires $feature to be available.
            #[allow(non_snake_case)]
            #[target_feature(enable = $feature)]
            unsafe fn md4xN(data: &[&[u8]; LANES]) -> [[u8; 16]; LANES] {
                let mut state = Md4State {
                    s: [
                        u32xN::splat(S[0]),
                        u32xN::splat(S[1]),
                        u32xN::splat(S[2]),
                        u32xN::splat(S[3]),
                    ],
                };
                let len = data[0].len();
                for ix in 1..LANES {
                    assert_eq!(len, data[ix].len());
                }
                let mut blocks = [u32xN::splat(0); 16];
                for block in 0..(len / 64) {
                    $load(&mut blocks, |lane| array_ref![&data[lane], 64 * block, 64]);
                    state.process_block(&blocks);
                }
                let remainder = len % 64;
                let bit_len = len as u64 * 8;
                {
                    let mut padded = [[0; 64]; LANES];
                    for lane in 0..LANES {
                        padded[lane][..remainder].copy_from_slice(&data[lane][len - remainder..]);
                        padded[lane][remainder] = 0x80;
                    }
                    $load(&mut blocks, |lane| &padded[lane]);
                    if remainder < 56 {
                        blocks[14] = u32xN::splat(bit_len as u32);
                        blocks[15] = u32xN::splat((bit_len >> 32) as u32);
                    }
                    state.process_block(&blocks);
                }
                if remainder >= 56 {
                    let mut blocks = [u32xN::splat(0); 16];
                    blocks[14] = u32xN::splat(bit_len as u32);
                    blocks[15] = u32xN::splat((bit_len >> 32) as u32);
                    state.process_block(&blocks);
                }
                let mut digests = [[0; 16]; LANES];
                for lane in 0..LANES {
                    let (a, b, c, d) = mut_array_refs!(&mut digests[lane], 4, 4, 4, 4);
                    *a = state.s[0].extract(lane).to_le_bytes();
                    *b = state.s[1].extract(lane).to_le_bytes();
                    *c = state.s[2].extract(lane).to_le_bytes();
                    *d = state.s[3].extract(lane).to_le_bytes();
                }
                digests
            }

            pub fn select() -> Option<Md4xN> {
                if is_x86_feature_detected!($feature) {
                    Some(Md4xN {
                        lanes: LANES,
                        fun: |data| {
                            let mut ret = [[0; 16]; MAX_LANES];
                            let (prefix, _) = mut_array_refs!(&mut ret, LANES, MAX_LANES-LANES);
                            // Unsafety: We just checked that $feature is available.
                            *prefix = unsafe { md4xN(array_ref![data, 0, LANES]) };
                            ret
                        }
                    })
                } else {
                    None
                }
            }
        );
    }

    mod lanes_4 {
        n_lanes!(
            packed_simd::u32x4,
            crate::md4::simd_transpose::load_16x4_sse2::<crate::md4::simd_transpose::LE, _>,
            "sse2"
        );
    }
    mod lanes_8 {
        n_lanes!(
            packed_simd::u32x8,
            crate::md4::simd_transpose::load_16x8::<crate::md4::simd_transpose::LE, _>,
            "avx2"
        );
    }

    impl Md4xN {
        /// Returns a SIMD implementation if one is available.
        pub fn select() -> Option<Md4xN> {
            lanes_8::select().or_else(lanes_4::select)
        }

        /// The number of digests this implementation calculates at once.
        pub fn lanes(&self) -> usize {
            self.lanes
        }

        /// Calculate the digest of `self.lanes()` equally-sized blocks of data.
        pub fn md4(&self, data: &[&[u8]]) -> [[u8; 16]; MAX_LANES] {
            (self.fun)(data)
        }
    }
}

pub fn md4_many<'a>(
    datas: impl ExactSizeIterator<Item = &'a [u8]>,
) -> impl ExactSizeIterator<Item = (&'a [u8], [u8; 16])> {
    struct SimdImpl<'a> {
        simd_impl: simd::Md4xN,
        buf: [(&'a [u8], [u8; 16]); simd::MAX_LANES],
        buf_len: usize,
    }
    struct It<'a, I: Iterator<Item = &'a [u8]>> {
        len: usize,
        inner: I,
        simd: Option<SimdImpl<'a>>,
    }
    impl<'a, I: Iterator<Item = &'a [u8]>> Iterator for It<'a, I> {
        type Item = (&'a [u8], [u8; 16]);
        #[allow(clippy::needless_range_loop)]
        fn next(&mut self) -> Option<Self::Item> {
            if let Some(simd) = &mut self.simd {
                if simd.buf_len == 0 && self.len >= simd.simd_impl.lanes() {
                    let mut datas: [&[u8]; simd::MAX_LANES] = [&[]; simd::MAX_LANES];
                    for ix in 0..simd.simd_impl.lanes() {
                        datas[ix] = self.inner.next().unwrap();
                    }
                    self.len -= simd.simd_impl.lanes();
                    let digests = simd.simd_impl.md4(&datas);
                    simd.buf_len = simd.simd_impl.lanes();
                    for lane in 0..simd.simd_impl.lanes() {
                        simd.buf[lane] = (datas[lane], digests[lane]);
                    }
                }
                if simd.buf_len > 0 {
                    let digest = simd.buf[simd.simd_impl.lanes() - simd.buf_len];
                    simd.buf_len -= 1;
                    return Some(digest);
                }
            }
            self.inner.next().map(|data| {
                self.len -= 1;
                (data, md4(data))
            })
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.len, Some(self.len))
        }
    }
    impl<'a, I: Iterator<Item = &'a [u8]>> ExactSizeIterator for It<'a, I> {
        fn len(&self) -> usize {
            self.len
        }
    }
    It {
        len: datas.len(),
        inner: datas,
        simd: simd::Md4xN::select().map(|simd_impl| SimdImpl {
            simd_impl,
            buf: [(&[] as &[_], [0; 16]); simd::MAX_LANES],
            buf_len: 0,
        }),
    }
}

#[test]
fn tests() {
    let test_vectors: &[(&[u8], [u8; 16])] = &[
        (
            b"",
            *b"\x31\xd6\xcf\xe0\xd1\x6a\xe9\x31\xb7\x3c\x59\xd7\xe0\xc0\x89\xc0",
        ),
        (
            b"a",
            *b"\xbd\xe5\x2c\xb3\x1d\xe3\x3e\x46\x24\x5e\x05\xfb\xdb\xd6\xfb\x24",
        ),
        (
            b"abc",
            *b"\xa4\x48\x01\x7a\xaf\x21\xd8\x52\x5f\xc1\x0a\xe8\x7a\xa6\x72\x9d",
        ),
        (
            b"message digest",
            *b"\xd9\x13\x0a\x81\x64\x54\x9f\xe8\x18\x87\x48\x06\xe1\xc7\x01\x4b",
        ),
        (
            b"abcdefghijklmnopqrstuvwxyz",
            *b"\xd7\x9e\x1c\x30\x8a\xa5\xbb\xcd\xee\xa8\xed\x63\xdf\x41\x2d\xa9",
        ),
        (
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            *b"\x04\x3f\x85\x82\xf2\x41\xdb\x35\x1c\xe6\x27\xe1\x53\xe7\xf0\xe4",
        ),
        (
            b"12345678901234567890123456789012345678901234567890123456789012345678901234567890",
            *b"\xe3\x3b\x4d\xdc\x9c\x38\xf2\x19\x9c\x3e\x7b\x16\x4f\xcc\x05\x36",
        ),
    ];

    for &(msg, expected) in test_vectors {
        assert_eq!(md4(msg), expected);
        if let Some(simd_impl) = simd::Md4xN::select() {
            assert_eq!(
                simd_impl.md4(&vec![msg; simd_impl.lanes()])[..simd_impl.lanes()],
                vec![expected; simd_impl.lanes()][..]
            );
        }
    }
}
