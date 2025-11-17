use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::Path;
use anyhow::{anyhow, Result};
use chrono::NaiveDateTime;
use crate::{RKAF_SIGNATURE, RKFW_SIGNATURE, UpdateHeader, RKAFP_MAGIC};

#[derive(Debug, Clone)]
pub struct RkfwInfo {
    pub version: String,
    pub code: u32,
    pub timestamp: i64,
    pub chip_family: String,
    pub chip_code: u8,
    pub boot_offset: u32,
    pub boot_size: u32,
    pub update_offset: u32,
    pub update_size: u32,
}

#[derive(Debug, Clone)]
pub struct PartitionInfo {
    pub name: String,
    pub path: String,
    pub flash_size: u32,
    pub flash_offset: u32,
    pub part_offset: u32,
    pub padded_size: u32,
    pub part_byte_count: u32,
}

#[derive(Debug, Clone)]
pub struct RkafInfo {
    pub manufacturer: String,
    pub model: String,
    pub filesize: u64,
    pub partitions: Vec<PartitionInfo>,
}

#[derive(Debug, Clone)]
pub enum UnpackResult {
    Rkfw(RkfwInfo),
    Rkaf(RkafInfo),
}

pub fn unpack_file(file_path: &str, dst_path: &str) -> Result<UnpackResult> {
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let signature = &buffer[0..4];
    match signature {
        RKAF_SIGNATURE => {
            let info = unpack_rkafp(file_path, dst_path)?;
            Ok(UnpackResult::Rkaf(info))
        }
        RKFW_SIGNATURE => {
            let info = unpack_rkfw(&buffer, dst_path)?;
            Ok(UnpackResult::Rkfw(info))
        }
        _ => {
            Err(anyhow!("Unknown signature: {:?}", signature))
        }
    }
}

fn unpack_rkfw(buf: &[u8], dst_path: &str) -> Result<RkfwInfo> {
    let mut chip: Option<&str> = None;

    println!("RKFW signature detected");

    let version_str = format!(
        "{}.{}.{}",
        buf[9],
        buf[8],
        ((buf[7] as u16) << 8) + buf[6] as u16
    );
    println!("version: {}", version_str);

    let code = u32::from_le_bytes([buf[0x0a], buf[0x0b], buf[0x0c], buf[0x0d]]);
    println!("code field: 0x{:08x}", code);

    let year = ((buf[0x0f] as u16) << 8) | (buf[0x0e] as u16);
    let month = buf[0x10];
    let day = buf[0x11];
    let hour = buf[0x12];
    let minute = buf[0x13];
    let second = buf[0x14];

    let date = chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32)
        .ok_or_else(|| anyhow!("Invalid date"))?;
    let time = chrono::NaiveTime::from_hms_opt(hour as u32, minute as u32, second as u32)
        .ok_or_else(|| anyhow!("Invalid time"))?;
    let dt = NaiveDateTime::new(date, time);
    let unix_timestamp = dt.and_utc().timestamp();

    println!(
        "date: {}-{:02}-{:02} {:02}:{:02}:{:02} (Unix timestamp: {})",
        year, month, day, hour, minute, second, unix_timestamp
    );

    let chip_code = buf[0x15];
    match chip_code {
        0x50 => chip = Some("RK29xx"),
        0x60 => chip = Some("RK30xx"),
        0x70 => chip = Some("RK31xx"),
        0x80 => chip = Some("RK32xx"),
        0x41 => chip = Some("RK3368"),
        0x36 => chip = Some("RK3326"),
        0x32 => chip = Some("RK3562"),
        0x38 => chip = Some("RK3566"),
        0x30 => chip = Some("PX30"),
        _ => println!(
            "You got a brand new chip ({:#x}), congratulations!!!",
            chip_code
        ),
    }

    let chip_name = chip.unwrap_or("unknown");
    println!("family: {}", chip_name);

    let boot_offset = get_u32_le(&buf[0x19..]);
    let boot_size: u32 = get_u32_le(&buf[0x1d..]);

    // if &buf[boot_offset as usize..boot_offset as usize + 4] != b"BOOT" {
    //     panic!("cannot find BOOT signature");
    // }

    println!(
        "{:08x}-{:08x} {:26} (size: {})",
        boot_offset,
        boot_offset + boot_size - 1,
        "BOOT",
        boot_size
    );
    std::fs::create_dir_all(dst_path)?;
    write_file(
        &Path::new(&format!("{}/BOOT", dst_path)),
        &buf[boot_offset as usize..boot_offset as usize + (boot_size as usize)],
    )?;

    let update_offset = get_u32_le(&buf[0x21..]);
    let update_size = get_u32_le(&buf[0x25..]);

    if &buf[update_offset as usize..update_offset as usize + 4] != b"RKAF" {
        panic!("cannot find embedded RKAF update.img");
    }

    println!(
        "{:08x}-{:08x} {:26} (size: {})",
        update_offset,
        update_offset + update_size - 1,
        "embedded-update.img",
        update_size
    );
    write_file(
        &Path::new(&format!("{}/embedded-update.img", dst_path)),
        &buf[update_offset as usize..update_offset as usize + update_size as usize],
    )?;

    Ok(RkfwInfo {
        version: version_str,
        code,
        timestamp: unix_timestamp,
        chip_family: chip_name.to_string(),
        chip_code,
        boot_offset,
        boot_size,
        update_offset,
        update_size,
    })
}

fn extract_file(fp: &mut File, offset: u64, len: u64, full_path: &str) -> Result<()> {
    println!("{:08x}-{:08x} {}", offset, len, full_path);
    let mut buffer = vec![0u8; 16 * 1024];
    let mut fp_out = File::create(full_path)?;

    fp.seek(std::io::SeekFrom::Start(offset))?;

    let mut remaining = len;

    while remaining > 0 {
        let read_len = std::cmp::min(remaining as usize, buffer.len());
        let read_bytes = fp.read(&mut buffer[..read_len])?;

        if read_bytes != read_len {
            return Err(anyhow!("Insufficient length in container image file"));
        }

        fp_out.write_all(&buffer[..read_len])?;

        remaining -= read_len as u64;
    }

    Ok(())
}

fn unpack_rkafp(file_path: &str, dst_path: &str) -> Result<RkafInfo> {
    use std::mem;

    let mut fp = File::open(file_path)?;
    let mut buf = vec![0u8; mem::size_of::<UpdateHeader>()];
    fp.read_exact(&mut buf)?;
    let header = UpdateHeader::from_bytes(buf.as_mut());
    let magic_str = std::str::from_utf8(&header.magic)?;
    if magic_str != RKAFP_MAGIC {
        return Err(anyhow!("Invalid header magic id"));
    }

    let filesize = fp.metadata()?.len();
    println!("Filesize: {}", filesize);
    if filesize - 4 != header.length as u64 {
        eprintln!("update_header.length cannot be correct, cannot check CRC");
    }
    std::fs::create_dir_all(format!("{}/Image", dst_path))?;
    // 安全地从null-terminated字符串中提取文本
    let manufacturer = std::ffi::CStr::from_bytes_until_nul(&header.manufacturer)
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let model = std::ffi::CStr::from_bytes_until_nul(&header.model)
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    println!("manufacturer: {}", manufacturer);
    println!("model: {}", model);

    // Save partition metadata for repacking
    let metadata_path = format!("{}/partition-metadata.txt", dst_path);
    let mut metadata_file = File::create(&metadata_path)?;
    let mut partitions = Vec::new();

    for i in 0..header.num_parts {
        let part = &header.parts[i as usize];
        // 安全地提取路径字符串
        if let Ok(cstr_path) = std::ffi::CStr::from_bytes_until_nul(&part.full_path) {
            let part_full_path = cstr_path.to_string_lossy();
            if part_full_path == "SELF" || part_full_path == "RESERVED" {
                continue;
            }

            let part_name = if let Ok(cstr_name) = std::ffi::CStr::from_bytes_until_nul(&part.name) {
                cstr_name.to_string_lossy().to_string()
            } else {
                String::new()
            };

            let flash_size = part.flash_size;
            let flash_offset = part.flash_offset;
            let part_offset = part.part_offset;
            let padded_size = part.padded_size;
            let part_byte_count = part.part_byte_count;

            writeln!(
                metadata_file,
                "{},{},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x}",
                part_name,
                part_full_path,
                flash_size,
                flash_offset,
                part_offset,
                padded_size,
                part_byte_count
            )?;

            partitions.push(PartitionInfo {
                name: part_name.clone(),
                path: part_full_path.to_string(),
                flash_size,
                flash_offset,
                part_offset,
                padded_size,
                part_byte_count,
            });

            let part_full_path = format!("{}/{}", dst_path, part_full_path);
            extract_file(
                &mut fp,
                part.part_offset as u64,
                part.part_byte_count as u64,
                &part_full_path,
            )?;
        }
    }

    println!("\nPartition metadata saved to: {}", metadata_path);

    Ok(RkafInfo {
        manufacturer,
        model,
        filesize,
        partitions,
    })
}

fn get_u32_le(slice: &[u8]) -> u32 {
    u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
}

fn write_file(path: &Path, buffer: &[u8]) -> Result<()> {
    let mut file = File::create(path)?;
    file.write_all(buffer)?;
    Ok(())
}
