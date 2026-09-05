//! ROM patch formats: IPS, UPS and BPS.
//!
//! A patch holds only the hack author's own changes, which is why the scene
//! distributes patches rather than ROMs. The base ROM is supplied by the user
//! and is never modified — patching produces a new buffer.
//!
//! UPS and BPS embed CRC32 checksums of the ROM they were built against, so a
//! mismatch can be reported precisely ("this needs Rev 0, yours is Rev 1")
//! instead of silently producing a corrupt game.

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Ips,
    Ups,
    Bps,
}

impl Format {
    pub fn as_str(&self) -> &'static str {
        match self {
            Format::Ips => "IPS",
            Format::Ups => "UPS",
            Format::Bps => "BPS",
        }
    }
}

/// What a patch declares about itself before it is applied.
#[derive(Debug, Clone)]
pub struct PatchInfo {
    pub format: Format,
    /// CRC32 of the ROM this patch expects. IPS carries no checksum.
    pub source_crc: Option<u32>,
    pub target_crc: Option<u32>,
    pub source_size: Option<u64>,
    pub target_size: Option<u64>,
}

pub fn detect_format(patch: &[u8]) -> Option<Format> {
    if patch.starts_with(b"PATCH") {
        Some(Format::Ips)
    } else if patch.starts_with(b"UPS1") {
        Some(Format::Ups)
    } else if patch.starts_with(b"BPS1") {
        Some(Format::Bps)
    } else {
        None
    }
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(data);
    h.finalize()
}

/// Read a patch's declared metadata without applying it.
pub fn inspect(patch: &[u8]) -> Result<PatchInfo> {
    let format = detect_format(patch)
        .ok_or_else(|| AppError::Other("Not an IPS, UPS or BPS patch".into()))?;

    match format {
        Format::Ips => Ok(PatchInfo {
            format,
            source_crc: None,
            target_crc: None,
            source_size: None,
            target_size: None,
        }),
        Format::Ups | Format::Bps => {
            if patch.len() < 16 {
                return Err(AppError::Other("Patch file is truncated".into()));
            }
            let footer = &patch[patch.len() - 12..];
            let source_crc = u32::from_le_bytes([footer[0], footer[1], footer[2], footer[3]]);
            let target_crc = u32::from_le_bytes([footer[4], footer[5], footer[6], footer[7]]);

            let mut cursor = 4usize;
            let source_size = read_varint(patch, &mut cursor)?;
            let target_size = read_varint(patch, &mut cursor)?;

            Ok(PatchInfo {
                format,
                source_crc: Some(source_crc),
                target_crc: Some(target_crc),
                source_size: Some(source_size),
                target_size: Some(target_size),
            })
        }
    }
}

/// Apply a patch to a ROM, verifying checksums where the format provides them.
pub fn apply(source: &[u8], patch: &[u8]) -> Result<Vec<u8>> {
    let info = inspect(patch)?;

    if let Some(expected) = info.source_crc {
        let actual = crc32(source);
        if actual != expected {
            return Err(AppError::Other(format!(
                "This patch expects a ROM with CRC32 {expected:08X}, but yours is {actual:08X}. \
                 It was most likely built against a different revision or region."
            )));
        }
    }

    let output = match info.format {
        Format::Ips => apply_ips(source, patch)?,
        Format::Ups => apply_ups(source, patch)?,
        Format::Bps => apply_bps(source, patch)?,
    };

    if let Some(expected) = info.target_crc {
        let actual = crc32(&output);
        if actual != expected {
            return Err(AppError::Other(format!(
                "Patching produced CRC32 {actual:08X} but the patch expects {expected:08X}. \
                 The patch or the ROM may be damaged."
            )));
        }
    }

    Ok(output)
}

// ------------------------------------------------------------------- IPS

fn apply_ips(source: &[u8], patch: &[u8]) -> Result<Vec<u8>> {
    let mut out = source.to_vec();
    let mut i = 5; // skip "PATCH"

    loop {
        if i + 3 > patch.len() {
            return Err(AppError::Other("IPS patch ended without an EOF marker".into()));
        }
        if &patch[i..i + 3] == b"EOF" {
            i += 3;
            // An optional 3-byte little-endian truncation length may follow.
            if i + 3 <= patch.len() {
                let truncate =
                    u32::from_be_bytes([0, patch[i], patch[i + 1], patch[i + 2]]) as usize;
                if truncate > 0 && truncate < out.len() {
                    out.truncate(truncate);
                }
            }
            break;
        }

        let offset = u32::from_be_bytes([0, patch[i], patch[i + 1], patch[i + 2]]) as usize;
        i += 3;

        if i + 2 > patch.len() {
            return Err(AppError::Other("IPS patch is truncated".into()));
        }
        let size = u16::from_be_bytes([patch[i], patch[i + 1]]) as usize;
        i += 2;

        if size == 0 {
            // RLE run: length, then the byte to repeat.
            if i + 3 > patch.len() {
                return Err(AppError::Other("IPS RLE record is truncated".into()));
            }
            let run = u16::from_be_bytes([patch[i], patch[i + 1]]) as usize;
            let value = patch[i + 2];
            i += 3;

            grow_to(&mut out, offset + run);
            out[offset..offset + run].fill(value);
        } else {
            if i + size > patch.len() {
                return Err(AppError::Other("IPS data record is truncated".into()));
            }
            grow_to(&mut out, offset + size);
            out[offset..offset + size].copy_from_slice(&patch[i..i + size]);
            i += size;
        }
    }

    Ok(out)
}

fn grow_to(buf: &mut Vec<u8>, len: usize) {
    if buf.len() < len {
        buf.resize(len, 0);
    }
}

// ---------------------------------------------------------- UPS / BPS varint

/// The variable-length integer shared by UPS and BPS: seven bits per byte,
/// low byte first, high bit marks the last byte, with each continuation
/// implicitly adding one more than the previous shift could represent.
fn read_varint(data: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value: u64 = 0;
    let mut shift: u64 = 1;
    loop {
        let byte = *data
            .get(*cursor)
            .ok_or_else(|| AppError::Other("Patch ended mid-number".into()))?;
        *cursor += 1;
        value += u64::from(byte & 0x7f) * shift;
        if byte & 0x80 != 0 {
            break;
        }
        shift <<= 7;
        value += shift;
    }
    Ok(value)
}

// ------------------------------------------------------------------- UPS

fn apply_ups(source: &[u8], patch: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = 4; // skip "UPS1"
    let source_size = read_varint(patch, &mut cursor)? as usize;
    let target_size = read_varint(patch, &mut cursor)? as usize;

    if source.len() != source_size {
        return Err(AppError::Other(format!(
            "This patch expects a {source_size}-byte ROM, but yours is {} bytes.",
            source.len()
        )));
    }

    let mut out = vec![0u8; target_size];
    let copy = source_size.min(target_size);
    out[..copy].copy_from_slice(&source[..copy]);

    let body_end = patch.len() - 12;
    let mut pos = 0usize;

    while cursor < body_end {
        let skip = read_varint(patch, &mut cursor)? as usize;
        pos += skip;

        // XOR bytes follow until a zero terminator.
        loop {
            let byte = *patch
                .get(cursor)
                .ok_or_else(|| AppError::Other("UPS patch is truncated".into()))?;
            cursor += 1;

            if byte == 0 {
                pos += 1;
                break;
            }
            if pos < out.len() {
                let base = if pos < source.len() { source[pos] } else { 0 };
                out[pos] = base ^ byte;
            }
            pos += 1;
        }
    }

    Ok(out)
}

// ------------------------------------------------------------------- BPS

const BPS_SOURCE_READ: u64 = 0;
const BPS_TARGET_READ: u64 = 1;
const BPS_SOURCE_COPY: u64 = 2;
const BPS_TARGET_COPY: u64 = 3;

fn apply_bps(source: &[u8], patch: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = 4; // skip "BPS1"
    let source_size = read_varint(patch, &mut cursor)? as usize;
    let target_size = read_varint(patch, &mut cursor)? as usize;
    let metadata_size = read_varint(patch, &mut cursor)? as usize;
    cursor += metadata_size;

    if source.len() != source_size {
        return Err(AppError::Other(format!(
            "This patch expects a {source_size}-byte ROM, but yours is {} bytes.",
            source.len()
        )));
    }

    let mut out = vec![0u8; target_size];
    let mut out_pos = 0usize;
    let mut source_rel: i64 = 0;
    let mut target_rel: i64 = 0;

    let body_end = patch
        .len()
        .checked_sub(12)
        .ok_or_else(|| AppError::Other("BPS patch is truncated".into()))?;

    while cursor < body_end {
        let data = read_varint(patch, &mut cursor)?;
        let command = data & 3;
        let length = ((data >> 2) + 1) as usize;

        if out_pos + length > target_size {
            return Err(AppError::Other("BPS patch overruns the target size".into()));
        }

        match command {
            BPS_SOURCE_READ => {
                for _ in 0..length {
                    let b = *source
                        .get(out_pos)
                        .ok_or_else(|| AppError::Other("BPS read past end of ROM".into()))?;
                    out[out_pos] = b;
                    out_pos += 1;
                }
            }
            BPS_TARGET_READ => {
                for _ in 0..length {
                    let b = *patch
                        .get(cursor)
                        .ok_or_else(|| AppError::Other("BPS patch is truncated".into()))?;
                    cursor += 1;
                    out[out_pos] = b;
                    out_pos += 1;
                }
            }
            BPS_SOURCE_COPY | BPS_TARGET_COPY => {
                let raw = read_varint(patch, &mut cursor)?;
                let delta = (raw >> 1) as i64;
                let signed = if raw & 1 != 0 { -delta } else { delta };

                if command == BPS_SOURCE_COPY {
                    source_rel += signed;
                    for _ in 0..length {
                        let idx = usize::try_from(source_rel).map_err(|_| {
                            AppError::Other("BPS source offset went negative".into())
                        })?;
                        let b = *source.get(idx).ok_or_else(|| {
                            AppError::Other("BPS copy past end of ROM".into())
                        })?;
                        out[out_pos] = b;
                        out_pos += 1;
                        source_rel += 1;
                    }
                } else {
                    target_rel += signed;
                    for _ in 0..length {
                        let idx = usize::try_from(target_rel).map_err(|_| {
                            AppError::Other("BPS target offset went negative".into())
                        })?;
                        if idx >= out_pos {
                            return Err(AppError::Other(
                                "BPS patch copies from data it has not written yet".into(),
                            ));
                        }
                        out[out_pos] = out[idx];
                        out_pos += 1;
                        target_rel += 1;
                    }
                }
            }
            _ => unreachable!("command is masked to two bits"),
        }
    }

    if out_pos != target_size {
        return Err(AppError::Other(format!(
            "BPS patch produced {out_pos} bytes but declared {target_size}"
        )));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_varint(out: &mut Vec<u8>, mut value: u64) {
        loop {
            let x = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(0x80 | x);
                break;
            }
            out.push(x);
            value -= 1;
        }
    }

    #[test]
    fn ips_replaces_bytes() {
        let source = vec![0u8; 8];
        let mut patch = b"PATCH".to_vec();
        patch.extend_from_slice(&[0, 0, 2]); // offset 2
        patch.extend_from_slice(&[0, 3]); // length 3
        patch.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        patch.extend_from_slice(b"EOF");

        let out = apply(&source, &patch).unwrap();
        assert_eq!(out, vec![0, 0, 0xAA, 0xBB, 0xCC, 0, 0, 0]);
    }

    #[test]
    fn ips_rle_run_and_extension() {
        let source = vec![1u8, 2, 3];
        let mut patch = b"PATCH".to_vec();
        patch.extend_from_slice(&[0, 0, 4]); // offset 4, past the end
        patch.extend_from_slice(&[0, 0]); // size 0 -> RLE
        patch.extend_from_slice(&[0, 3]); // run of 3
        patch.push(0xFF);
        patch.extend_from_slice(b"EOF");

        let out = apply(&source, &patch).unwrap();
        assert_eq!(out, vec![1, 2, 3, 0, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn varint_round_trips() {
        for value in [0u64, 1, 127, 128, 255, 4096, 1 << 20, 123_456_789] {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            let mut cursor = 0;
            assert_eq!(read_varint(&buf, &mut cursor).unwrap(), value, "value {value}");
            assert_eq!(cursor, buf.len());
        }
    }

    /// A BPS built only from TargetRead actions rewrites the file wholesale.
    #[test]
    fn bps_target_read_rewrites() {
        let source = vec![9u8; 4];
        let target = vec![1u8, 2, 3, 4, 5];

        let mut body = Vec::new();
        write_varint(&mut body, ((target.len() as u64 - 1) << 2) | BPS_TARGET_READ);
        body.extend_from_slice(&target);

        let patch = build_bps(&source, &target, body);
        assert_eq!(apply(&source, &patch).unwrap(), target);
    }

    /// SourceRead copies straight through; TargetRead supplies the new tail.
    #[test]
    fn bps_source_read_then_target_read() {
        let source = vec![1u8, 2, 3, 4];
        let target = vec![1u8, 2, 0xEE, 0xFF];

        let mut body = Vec::new();
        write_varint(&mut body, (1 << 2) | BPS_SOURCE_READ); // 2 bytes from source
        write_varint(&mut body, (1 << 2) | BPS_TARGET_READ); // 2 literal bytes
        body.extend_from_slice(&[0xEE, 0xFF]);

        let patch = build_bps(&source, &target, body);
        assert_eq!(apply(&source, &patch).unwrap(), target);
    }

    #[test]
    fn bps_source_copy_reaches_backwards() {
        let source = vec![0xA0, 0xA1, 0xA2, 0xA3];
        let target = vec![0xA2, 0xA3];

        let mut body = Vec::new();
        write_varint(&mut body, (1 << 2) | BPS_SOURCE_COPY);
        write_varint(&mut body, 2 << 1); // +2, positive

        let patch = build_bps(&source, &target, body);
        assert_eq!(apply(&source, &patch).unwrap(), target);
    }

    #[test]
    fn wrong_rom_is_rejected_by_crc() {
        let source = vec![1u8, 2, 3, 4];
        let target = vec![1u8, 2, 3, 4];
        let mut body = Vec::new();
        write_varint(&mut body, (3 << 2) | BPS_SOURCE_READ);
        let patch = build_bps(&source, &target, body);

        let wrong = vec![9u8, 9, 9, 9];
        let err = apply(&wrong, &patch).unwrap_err().to_string();
        assert!(err.contains("CRC32"), "unexpected error: {err}");
    }

    #[test]
    fn ups_xors_and_reports_size_mismatch() {
        let source = vec![0x10, 0x20, 0x30];
        let target = vec![0x10, 0x25, 0x30];

        let mut patch = b"UPS1".to_vec();
        write_varint(&mut patch, source.len() as u64);
        write_varint(&mut patch, target.len() as u64);
        write_varint(&mut patch, 1); // skip to index 1
        patch.push(0x20 ^ 0x25);
        patch.push(0); // end of this XOR run
        patch.extend_from_slice(&crc32(&source).to_le_bytes());
        patch.extend_from_slice(&crc32(&target).to_le_bytes());
        let so_far = crc32(&patch);
        patch.extend_from_slice(&so_far.to_le_bytes());

        assert_eq!(apply(&source, &patch).unwrap(), target);
    }

    fn build_bps(source: &[u8], target: &[u8], body: Vec<u8>) -> Vec<u8> {
        let mut patch = b"BPS1".to_vec();
        write_varint(&mut patch, source.len() as u64);
        write_varint(&mut patch, target.len() as u64);
        write_varint(&mut patch, 0); // no metadata
        patch.extend_from_slice(&body);
        patch.extend_from_slice(&crc32(source).to_le_bytes());
        patch.extend_from_slice(&crc32(target).to_le_bytes());
        let so_far = crc32(&patch);
        patch.extend_from_slice(&so_far.to_le_bytes());
        patch
    }
}
