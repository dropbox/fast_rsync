const CRC_MAGIC: u16 = 31;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Crc(pub u32);

impl Crc {
    pub const SIZE: usize = 4;

    #[inline]
    pub fn to_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_be_bytes()
    }

    #[inline]
    pub fn from_bytes(b: [u8; Self::SIZE]) -> Self {
        Crc(u32::from_be_bytes(b))
    }

    #[inline]
    fn split(self) -> (u16, u16) {
        (self.0 as u16, (self.0 >> 16) as u16)
    }

    #[inline]
    fn combine(s1: u16, s2: u16) -> Crc {
        Crc(s1 as u32 | ((s2 as u32) << 16))
    }

    #[inline]
    pub fn new() -> Crc {
        Crc(0)
    }

    #[allow(dead_code)]
    pub fn rollout(self, size: u32, old_byte: u8) -> Crc {
        let size = size as u16;
        let old_byte = old_byte as u16;
        let (mut s1, mut s2) = self.split();
        s1 = s1.wrapping_sub(old_byte.wrapping_add(CRC_MAGIC));
        s2 = s2.wrapping_sub(size.wrapping_mul(old_byte + CRC_MAGIC));
        Crc::combine(s1, s2)
    }

    #[inline]
    pub fn rotate(self, size: u32, old_byte: u8, new_byte: u8) -> Crc {
        let size = size as u16;
        let old_byte = old_byte as u16;
        let new_byte = new_byte as u16;
        let (mut s1, mut s2) = self.split();
        s1 = s1.wrapping_add(new_byte).wrapping_sub(old_byte);
        s2 = s2
            .wrapping_add(s1)
            .wrapping_sub(size.wrapping_mul(old_byte.wrapping_add(CRC_MAGIC)));
        Crc::combine(s1, s2)
    }

    #[allow(dead_code)]
    pub fn rollin(self, new_byte: u8) -> Crc {
        let (mut s1, mut s2) = self.split();
        s1 = s1.wrapping_add(new_byte as u16);
        s2 = s2.wrapping_add(s1);
        s1 = s1.wrapping_add(CRC_MAGIC);
        s2 = s2.wrapping_add(CRC_MAGIC);
        Crc::combine(s1, s2)
    }

    pub fn update(self, buf: &[u8]) -> Crc {
        macro_rules! imp {
            ($($x:tt)*) => {$($x)* (init: Crc, buf: &[u8]) -> Crc {
                let (mut s1, mut s2) = init.split();
                let len = buf.len() as u32;
                s2 = s2.wrapping_add(s1.wrapping_mul(len as u16));
                for (idx, &byte) in buf.iter().enumerate() {
                    s1 = s1.wrapping_add(byte as u16);
                    s2 = s2.wrapping_add(
                        (byte as u16).wrapping_mul((len as u16).wrapping_sub(idx as u16)),
                    );
                }
                s1 = s1.wrapping_add((len as u16).wrapping_mul(CRC_MAGIC));
                s2 = s2.wrapping_add(
                    ((len.wrapping_mul(len.wrapping_add(1)) / 2) as u16).wrapping_mul(CRC_MAGIC),
                );
                Crc::combine(s1, s2)
            }};
        }
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                imp!(#[target_feature(enable = "avx2")] unsafe fn imp_avx2);
                unsafe {
                    return imp_avx2(self, buf);
                }
            }
            if is_x86_feature_detected!("sse2") {
                imp!(#[target_feature(enable = "sse2")] unsafe fn imp_sse2);
                unsafe {
                    return imp_sse2(self, buf);
                }
            }
        }
        imp!(fn imp_baseline);
        imp_baseline(self, buf)
    }

    /// Like `Crc::update`, but not autovectorizable.
    #[allow(dead_code)]
    pub fn basic_update(self, buf: &[u8]) -> Crc {
        let (mut s1, mut s2) = self.split();
        for &byte in buf {
            s1 = s1.wrapping_add(byte as u16);
            s2 = s2.wrapping_add(s1);
        }
        let len = buf.len() as u32;
        s1 = s1.wrapping_add((len as u16).wrapping_mul(CRC_MAGIC));
        s2 = s2.wrapping_add(
            ((len.wrapping_mul(len.wrapping_add(1)) / 2) as u16).wrapping_mul(CRC_MAGIC),
        );
        Crc::combine(s1, s2)
    }
}

#[cfg(test)]
mod tests {
    use super::Crc;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn rollin_one(initial: u32, buf: Vec<u8>) -> bool {
        let sum1 = Crc(initial).update(&buf);
        let sum2 = buf.iter().copied().fold(Crc(initial), Crc::rollin);
        sum1 == sum2
    }

    #[quickcheck]
    fn optimized_update(initial: u32, buf: Vec<u8>) -> bool {
        let sum1 = Crc(initial).update(&buf);
        let sum2 = Crc(initial).basic_update(&buf);
        sum1 == sum2
    }

    #[quickcheck]
    fn update_twice(initial: u32, mut buf1: Vec<u8>, buf2: Vec<u8>) -> bool {
        let sum1 = Crc(initial).update(&buf1).update(&buf2);
        buf1.extend(&buf2);
        let sum2 = Crc(initial).update(&buf1);
        sum1 == sum2
    }

    #[quickcheck]
    fn rotate_one(mut buf: Vec<u8>, byte: u8) -> bool {
        if buf.is_empty() {
            return true;
        }
        let sum1 = Crc::new()
            .update(&buf)
            .rotate(buf.len() as u32, buf[0], byte);
        buf.push(byte);
        let sum2 = Crc::new().update(&buf[1..]);
        sum1 == sum2
    }

    #[quickcheck]
    fn rollout_one(buf: Vec<u8>) -> bool {
        if buf.is_empty() {
            return true;
        }
        let sum1 = Crc::new().update(&buf).rollout(buf.len() as u32, buf[0]);
        let sum2 = Crc::new().update(&buf[1..]);
        sum1 == sum2
    }
}
