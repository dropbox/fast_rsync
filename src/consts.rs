pub const MD4_MAGIC: u32 = 0x72730136;
pub const BLAKE2_MAGIC: u32 = 0x72730137;
pub const DELTA_MAGIC: u32 = 0x72730236;

pub const RS_OP_END: u8 = 0;

pub const RS_OP_LITERAL_1: u8 = 0x1;
pub const RS_OP_LITERAL_64: u8 = 0x40;

pub const RS_OP_LITERAL_N1: u8 = 0x41;
pub const RS_OP_LITERAL_N2: u8 = 0x42;
pub const RS_OP_LITERAL_N4: u8 = 0x43;
pub const RS_OP_LITERAL_N8: u8 = 0x44;

pub const RS_OP_COPY_N1_N1: u8 = 0x45;
// pub const RS_OP_COPY_N1_N2: u8 = 0x46;
// pub const RS_OP_COPY_N1_N4: u8 = 0x47;
// pub const RS_OP_COPY_N1_N8: u8 = 0x48;
// pub const RS_OP_COPY_N2_N1: u8 = 0x49;
// pub const RS_OP_COPY_N2_N2: u8 = 0x4a;
// pub const RS_OP_COPY_N2_N4: u8 = 0x4b;
// pub const RS_OP_COPY_N2_N8: u8 = 0x4c;
// pub const RS_OP_COPY_N4_N1: u8 = 0x4d;
// pub const RS_OP_COPY_N4_N2: u8 = 0x4e;
// pub const RS_OP_COPY_N4_N4: u8 = 0x4f;
// pub const RS_OP_COPY_N4_N8: u8 = 0x50;
// pub const RS_OP_COPY_N8_N1: u8 = 0x51;
// pub const RS_OP_COPY_N8_N2: u8 = 0x52;
// pub const RS_OP_COPY_N8_N4: u8 = 0x53;
pub const RS_OP_COPY_N8_N8: u8 = 0x54;
