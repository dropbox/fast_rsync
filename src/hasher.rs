use crate::crc::Crc;
use std::hash::{BuildHasherDefault, Hash, Hasher};

/// A very simple hasher designed for hashing `Crc`.
#[derive(Default)]
pub struct CrcHasher {
    state: u32,
}

impl Hasher for CrcHasher {
    fn write(&mut self, _: &[u8]) {
        panic!("not designed for general writes");
    }
    #[inline]
    fn write_u32(&mut self, val: u32) {
        assert_eq!(self.state, 0, "can't hash more than one u32");
        self.state = val;
    }
    #[cfg(target_pointer_width = "64")]
    #[inline]
    fn finish(&self) -> u64 {
        // the avalanche function from xxhash
        let mut val = self.state as u64;
        val ^= val >> 33;
        val = val.wrapping_mul(0xC2B2AE3D27D4EB4F);
        val ^= val >> 29;
        val = val.wrapping_mul(0x165667B19E3779F9);
        val ^= val >> 32;
        val
    }
    #[cfg(target_pointer_width = "32")]
    #[inline]
    fn finish(&self) -> u64 {
        let mut val = self.state;
        val ^= val >> 15;
        val = val.wrapping_mul(0x85EBCA77);
        val ^= val >> 13;
        val = val.wrapping_mul(0xC2B2AE3D);
        val ^= val >> 16;
        val as u64
    }
}

pub type BuildCrcHasher = BuildHasherDefault<CrcHasher>;

impl Hash for Crc {
    // This `#[inline]` is important for performance without LTO - the derived implementation doesn't always get inlined.
    #[inline]
    fn hash<H: Hasher>(&self, hash: &mut H) {
        hash.write_u32(self.0);
    }
}
