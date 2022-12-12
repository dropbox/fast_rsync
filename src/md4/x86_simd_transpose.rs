//! Utilities for loading and transposing data from memory on x86 architectures.
//! This is useful for SPMD-style operations.
use arrayref::array_ref;

use self::arch::{
    __m128i, __m256i, _mm256_castsi128_si256, _mm256_inserti128_si256, _mm256_unpackhi_epi32,
    _mm256_unpackhi_epi64, _mm256_unpacklo_epi32, _mm256_unpacklo_epi64, _mm_loadu_si128,
    _mm_unpackhi_epi32, _mm_unpackhi_epi64, _mm_unpacklo_epi32, _mm_unpacklo_epi64,
};
#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

#[inline(always)]
/// Loads four u32s (little-endian), potentially unaligned
unsafe fn load_u32x4(slice: &[u8; 16]) -> __m128i {
    _mm_loadu_si128(slice as *const [u8; 16] as *const __m128i)
}

/// Load 32 bytes (1 u32x8) out of each lane of `data`, transposed.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn load_transpose8(data: [&[u8; 32]; 8]) -> [__m256i; 8] {
    #[inline(always)]
    /// Concatenate two u32x4s into a single u32x8
    unsafe fn cat2x4(a: __m128i, b: __m128i) -> __m256i {
        // `vinserti128`
        _mm256_inserti128_si256(_mm256_castsi128_si256(a), b, 1)
    }

    let l04 = cat2x4(
        load_u32x4(array_ref![data[0], 0, 16]),
        load_u32x4(array_ref![data[4], 0, 16]),
    );
    let l15 = cat2x4(
        load_u32x4(array_ref![data[1], 0, 16]),
        load_u32x4(array_ref![data[5], 0, 16]),
    );
    let l26 = cat2x4(
        load_u32x4(array_ref![data[2], 0, 16]),
        load_u32x4(array_ref![data[6], 0, 16]),
    );
    let l37 = cat2x4(
        load_u32x4(array_ref![data[3], 0, 16]),
        load_u32x4(array_ref![data[7], 0, 16]),
    );
    let h04 = cat2x4(
        load_u32x4(array_ref![data[0], 16, 16]),
        load_u32x4(array_ref![data[4], 16, 16]),
    );
    let h15 = cat2x4(
        load_u32x4(array_ref![data[1], 16, 16]),
        load_u32x4(array_ref![data[5], 16, 16]),
    );
    let h26 = cat2x4(
        load_u32x4(array_ref![data[2], 16, 16]),
        load_u32x4(array_ref![data[6], 16, 16]),
    );
    let h37 = cat2x4(
        load_u32x4(array_ref![data[3], 16, 16]),
        load_u32x4(array_ref![data[7], 16, 16]),
    );
    // [data[0][0], data[1][0], data[0][1], data[1][1], data[4][0], data[5][0], data[4][1], data[5][1]]
    let a0145 = _mm256_unpacklo_epi32(l04, l15);
    // [data[0][2], data[1][2], data[0][3], data[1][3], data[4][2], data[5][2], data[4][3], data[5][3]]
    let b0145 = _mm256_unpackhi_epi32(l04, l15);
    let a2367 = _mm256_unpacklo_epi32(l26, l37);
    let b2367 = _mm256_unpackhi_epi32(l26, l37);
    let c0145 = _mm256_unpacklo_epi32(h04, h15);
    let d0145 = _mm256_unpackhi_epi32(h04, h15);
    let c2367 = _mm256_unpacklo_epi32(h26, h37);
    let d2367 = _mm256_unpackhi_epi32(h26, h37);
    [
        _mm256_unpacklo_epi64(a0145, a2367),
        _mm256_unpackhi_epi64(a0145, a2367),
        _mm256_unpacklo_epi64(b0145, b2367),
        _mm256_unpackhi_epi64(b0145, b2367),
        _mm256_unpacklo_epi64(c0145, c2367),
        _mm256_unpackhi_epi64(c0145, c2367),
        _mm256_unpacklo_epi64(d0145, d2367),
        _mm256_unpackhi_epi64(d0145, d2367),
    ]
}

macro_rules! get_blocks {
    ($data: ident, ($($lane: tt)*), $from: expr, $width: expr) => ([$(array_ref![&$data($lane), $from, $width]),*]);
}

#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn load_16x8_avx2<'a, F: Fn(usize) -> &'a [u8; 64]>(data: F) -> [__m256i; 16] {
    // We're using transmute to concatenate arrays; not ideal, but it works
    core::mem::transmute::<[[__m256i; 8]; 2], [__m256i; 16]>([
        load_transpose8(get_blocks!(data, (0 1 2 3 4 5 6 7), 0, 32)),
        load_transpose8(get_blocks!(data, (0 1 2 3 4 5 6 7), 32, 32)),
    ])
}

/// Load 16 bytes (1 u32x4) out of each lane of `data`, transposed.
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn load_transpose4(data: [&[u8; 16]; 4]) -> [__m128i; 4] {
    let i0 = load_u32x4(data[0]);
    let i1 = load_u32x4(data[1]);
    let i2 = load_u32x4(data[2]);
    let i3 = load_u32x4(data[3]);
    // [data[0][0], data[1][0], data[0][1], data[1][1]]
    let l01 = _mm_unpacklo_epi32(i0, i1);
    // [data[0][2], data[1][2], data[0][3], data[1][3]]
    let h01 = _mm_unpackhi_epi32(i0, i1);
    let l23 = _mm_unpacklo_epi32(i2, i3);
    let h23 = _mm_unpackhi_epi32(i2, i3);
    [
        _mm_unpacklo_epi64(l01, l23),
        _mm_unpackhi_epi64(l01, l23),
        _mm_unpacklo_epi64(h01, h23),
        _mm_unpackhi_epi64(h01, h23),
    ]
}

#[inline]
#[target_feature(enable = "sse2")]
pub unsafe fn load_16x4_sse2<'a, F: Fn(usize) -> &'a [u8; 64]>(data: F) -> [__m128i; 16] {
    core::mem::transmute::<[[__m128i; 4]; 4], [__m128i; 16]>([
        load_transpose4(get_blocks!(data, (0 1 2 3), 0, 16)),
        load_transpose4(get_blocks!(data, (0 1 2 3), 16, 16)),
        load_transpose4(get_blocks!(data, (0 1 2 3), 32, 16)),
        load_transpose4(get_blocks!(data, (0 1 2 3), 48, 16)),
    ])
}
