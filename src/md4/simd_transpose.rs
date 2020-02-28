//! Utilities for loading and transposing data from memory.
//! This is useful for SPMD-style operations.
use arrayref::{array_ref, array_refs, mut_array_refs};
use packed_simd::{shuffle, u32x4, u32x8};

pub struct LE;

pub trait Endian {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn load(x: &[u8; 4]) -> u32;
}

impl Endian for LE {
    #[inline(always)]
    fn load(x: &[u8; 4]) -> u32 {
        u32::from_le_bytes(*x)
    }
}

/// `movdqu`
#[inline(always)]
fn load_u32x4<E: Endian>(slice: &[u8; 16]) -> u32x4 {
    let (a, b, c, d) = array_refs![slice, 4, 4, 4, 4];
    u32x4::new(E::load(a), E::load(b), E::load(c), E::load(d))
}

/// Load 32 bytes (1 u32x8) out of each lane of `data`, transposed.
#[target_feature(enable = "avx2")]
unsafe fn load_transpose8<E: Endian>(data: [&[u8; 32]; 8], out: &mut [u32x8; 8]) {
    #[inline(always)]
    fn cat2x4(a: u32x4, b: u32x4) -> u32x8 {
        // `vinserti32x4`
        shuffle!(a, b, [0, 1, 2, 3, 4, 5, 6, 7])
    }

    #[inline(always)]
    fn unpacklo2x4(a: u32x8, b: u32x8) -> u32x8 {
        // `vpunpckldq`
        shuffle!(a, b, [0, 8, 1, 9, 4, 12, 5, 13])
    }

    #[inline(always)]
    fn unpackhi2x4(a: u32x8, b: u32x8) -> u32x8 {
        // `vpunpckhdq`
        shuffle!(a, b, [2, 10, 3, 11, 6, 14, 7, 15])
    }

    let l04 = cat2x4(
        load_u32x4::<E>(array_ref![data[0], 0, 16]),
        load_u32x4::<E>(array_ref![data[4], 0, 16]),
    );
    let l15 = cat2x4(
        load_u32x4::<E>(array_ref![data[1], 0, 16]),
        load_u32x4::<E>(array_ref![data[5], 0, 16]),
    );
    let l26 = cat2x4(
        load_u32x4::<E>(array_ref![data[2], 0, 16]),
        load_u32x4::<E>(array_ref![data[6], 0, 16]),
    );
    let l37 = cat2x4(
        load_u32x4::<E>(array_ref![data[3], 0, 16]),
        load_u32x4::<E>(array_ref![data[7], 0, 16]),
    );
    let h04 = cat2x4(
        load_u32x4::<E>(array_ref![data[0], 16, 16]),
        load_u32x4::<E>(array_ref![data[4], 16, 16]),
    );
    let h15 = cat2x4(
        load_u32x4::<E>(array_ref![data[1], 16, 16]),
        load_u32x4::<E>(array_ref![data[5], 16, 16]),
    );
    let h26 = cat2x4(
        load_u32x4::<E>(array_ref![data[2], 16, 16]),
        load_u32x4::<E>(array_ref![data[6], 16, 16]),
    );
    let h37 = cat2x4(
        load_u32x4::<E>(array_ref![data[3], 16, 16]),
        load_u32x4::<E>(array_ref![data[7], 16, 16]),
    );
    // [data[0][0], data[1][0], data[0][1], data[1][1], data[4][0], data[5][0], data[4][1], data[5][1]]
    let a0145 = unpacklo2x4(l04, l15);
    // [data[0][2], data[1][2], data[0][3], data[1][3], data[4][2], data[5][2], data[4][3], data[5][3]]
    let b0145 = unpackhi2x4(l04, l15);
    let a2367 = unpacklo2x4(l26, l37);
    let b2367 = unpackhi2x4(l26, l37);
    let c0145 = unpacklo2x4(h04, h15);
    let d0145 = unpackhi2x4(h04, h15);
    let c2367 = unpacklo2x4(h26, h37);
    let d2367 = unpackhi2x4(h26, h37);
    out[0] = shuffle!(a0145, a2367, [0, 1, 8, 9, 4, 5, 12, 13]);
    out[1] = shuffle!(a0145, a2367, [2, 3, 10, 11, 6, 7, 14, 15]);
    out[2] = shuffle!(b0145, b2367, [0, 1, 8, 9, 4, 5, 12, 13]);
    out[3] = shuffle!(b0145, b2367, [2, 3, 10, 11, 6, 7, 14, 15]);
    out[4] = shuffle!(c0145, c2367, [0, 1, 8, 9, 4, 5, 12, 13]);
    out[5] = shuffle!(c0145, c2367, [2, 3, 10, 11, 6, 7, 14, 15]);
    out[6] = shuffle!(d0145, d2367, [0, 1, 8, 9, 4, 5, 12, 13]);
    out[7] = shuffle!(d0145, d2367, [2, 3, 10, 11, 6, 7, 14, 15]);
}

macro_rules! get_blocks {
    ($data: ident, ($($lane: tt)*), $from: expr, $width: expr) => ([$(array_ref![&$data($lane), $from, $width]),*]);
}

#[inline]
#[target_feature(enable = "avx2")]
// use a return pointer to avoid initialization cost (LLVM can't figure
// out that it's elideable)
pub unsafe fn load_16x8<'a, E: Endian, F: Fn(usize) -> &'a [u8; 64]>(
    blocks: &mut [u32x8; 16],
    data: F,
) {
    let (a, b) = mut_array_refs![blocks, 8, 8];
    load_transpose8::<E>(get_blocks!(data, (0 1 2 3 4 5 6 7), 0, 32), a);
    load_transpose8::<E>(get_blocks!(data, (0 1 2 3 4 5 6 7), 32, 32), b);
}

macro_rules! load_16x4 {
    ($f: ident, $feature: tt) => {
        #[inline]
        #[target_feature(enable = $feature)]
        pub unsafe fn $f<'a, E: Endian, F: Fn(usize) -> &'a [u8; 64]>(
            blocks: &mut [u32x4; 16],
            data: F,
        ) {
            /// Load 16 bytes (1 u32x4) out of each lane of `data`, transposed.
            #[target_feature(enable = $feature)]
            unsafe fn load_transpose4<E: Endian>(data: [&[u8; 16]; 4], out: &mut [u32x4; 4]) {
                #[inline(always)]
                fn unpacklo4(a: u32x4, b: u32x4) -> u32x4 {
                    // `punpckldq`
                    shuffle!(a, b, [0, 4, 1, 5])
                }

                #[inline(always)]
                fn unpackhi4(a: u32x4, b: u32x4) -> u32x4 {
                    // `vpunpckhdq`
                    shuffle!(a, b, [2, 6, 3, 7])
                }

                let i0 = load_u32x4::<E>(data[0]);
                let i1 = load_u32x4::<E>(data[1]);
                let i2 = load_u32x4::<E>(data[2]);
                let i3 = load_u32x4::<E>(data[3]);
                // [data[0][0], data[1][0], data[0][1], data[1][1]]
                let l01 = unpacklo4(i0, i1);
                // [data[0][2], data[1][2], data[0][3], data[1][3]]
                let h01 = unpackhi4(i0, i1);
                let l23 = unpacklo4(i2, i3);
                let h23 = unpackhi4(i2, i3);
                out[0] = shuffle!(l01, l23, [0, 1, 4, 5]);
                out[1] = shuffle!(l01, l23, [2, 3, 6, 7]);
                out[2] = shuffle!(h01, h23, [0, 1, 4, 5]);
                out[3] = shuffle!(h01, h23, [2, 3, 6, 7]);
            }

            let (a, b, c, d) = mut_array_refs![blocks, 4, 4, 4, 4];
            load_transpose4::<E>(get_blocks!(data, (0 1 2 3), 0, 16), a);
            load_transpose4::<E>(get_blocks!(data, (0 1 2 3), 16, 16), b);
            load_transpose4::<E>(get_blocks!(data, (0 1 2 3), 32, 16), c);
            load_transpose4::<E>(get_blocks!(data, (0 1 2 3), 48, 16), d);
        }
    };
}

load_16x4!(load_16x4_sse2, "sse2");
