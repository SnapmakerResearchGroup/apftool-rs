use std::mem;
mod pack;
mod unpack;

pub use pack::{pack_rkfw, pack_rkaf, chip_name_to_code, PackRkfwResult, PackRkafResult};
pub use unpack::{unpack_file, UnpackResult, RkfwInfo, RkafInfo, PartitionInfo};

pub const RKAFP_MAGIC: &str = "RKAF";
pub const PARM_MAGIC: &str = "PARM";
pub const MAX_PARTS: usize = 16;
pub const MAX_NAME_LEN: usize = 32;
const MAX_FULL_PATH_LEN: usize = 60;
const MAX_MODEL_LEN: usize = 34;
const MAX_ID_LEN: usize = 30;
const MAX_MANUFACTURER_LEN: usize = 56;
pub const RKAF_SIGNATURE: &[u8] = b"RKAF";
pub const RKFW_SIGNATURE: &[u8] = b"RKFW";
pub const RKFP_SIGNATURE: &[u8] = b"RKFP";

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct UpdatePart {
    pub name: [u8; MAX_NAME_LEN],
    pub full_path: [u8; MAX_FULL_PATH_LEN],
    pub flash_size: u32,
    pub part_offset: u32,
    pub flash_offset: u32,
    pub padded_size: u32,
    pub part_byte_count: u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct UpdateHeader {
    pub magic: [u8; 4],
    pub length: u32,
    pub model: [u8; MAX_MODEL_LEN],
    id: [u8; MAX_ID_LEN],
    pub manufacturer: [u8; MAX_MANUFACTURER_LEN],
    pub unknown1: u32,
    pub version: u32,
    pub num_parts: u32,
    pub parts: [UpdatePart; MAX_PARTS],
    reserved: [u8; 116],
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct ParamHeader {
    magic: [u8; 4],
    length: u32,
}

impl UpdateHeader {
    pub fn default() -> Self {
        Self {
            magic: [0u8; 4],
            length: 0,
            model: [0u8; MAX_MODEL_LEN],
            id: [0u8; MAX_ID_LEN],
            manufacturer: [0u8; MAX_MANUFACTURER_LEN],
            unknown1: 0,
            version: 0,
            num_parts: 0,
            parts: [UpdatePart::default(); MAX_PARTS],
            reserved: [0u8; 116],
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> &UpdateHeader {
        unsafe { mem::transmute(bytes.as_ptr()) }
    }

    pub fn to_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<UpdateHeader>()) }
    }
}

impl UpdatePart {
    pub fn default() -> Self {
        Self {
            name: [0u8; MAX_NAME_LEN],
            full_path: [0u8; MAX_FULL_PATH_LEN],
            flash_size: 0,
            part_offset: 0,
            flash_offset: 0,
            padded_size: 0,
            part_byte_count: 0,
        }
    }
}

pub fn info_and_fatal(is_fatal: bool, message: String) {
    if is_fatal {
        eprint!("rkunpack: fatal: ");
    } else {
        eprint!("rkunpack: info: ");
    }
    eprintln!("{}", message);
    if is_fatal {
        std::process::exit(1);
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        info_and_fatal(false, format!($($arg)*));
    };
}

#[macro_export]
macro_rules! fatal {
    ($($arg:tt)*) => {
        info_and_fatal(true, format!($($arg)*));
    };
}

pub unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    core::slice::from_raw_parts(
        (p as *const T) as *const u8,
        mem::size_of::<T>(),
    )
}
