//! Identifying a system from a file's own bytes.
//!
//! Extensions and filenames are what somebody typed. A magic number is what
//! the machine actually wrote, so where the two disagree the bytes win.
//!
//! This exists for the ambiguous cases. `.sfc` or `.nsp` already belong to
//! exactly one system and settle themselves; `.iso`, `.bin` and `.chd` belong
//! to a dozen and, until now, fell through to guessing from folder and file
//! names. A disc image says what it is in its first few hundred bytes.
//!
//! Only the first kilobyte is read, which is where every signature here
//! lives, so this costs one short read rather than a scan of the file.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub struct Signature {
    pub offset: usize,
    pub magic: &'static [u8],
    pub platform: &'static str,
}

/// Magic numbers, longest and most specific first.
///
/// The Nintendo entries were read off real files rather than taken on trust:
/// a Wii dump carries `WBFS` at 0 and wraps the disc header at 0x200, where
/// `5D1C9EA3` sits at +0x18 next to the disc ID; a Switch NSP opens with
/// `PFS0`. The Sega and GameCube entries come from format documentation.
///
/// A wrong entry here is bounded: signatures are only consulted when the
/// extension did not already settle the system, so the worst a bad one can do
/// is mis-resolve a file that was going to be guessed at anyway.
pub const SIGNATURES: &[Signature] = &[
    // --- Nintendo
    Signature { offset: 0, magic: b"NES\x1a", platform: "nes" },
    Signature { offset: 0, magic: b"PFS0", platform: "switch" },
    Signature { offset: 0x100, magic: b"HEAD", platform: "switch" },
    Signature { offset: 0, magic: b"WBFS", platform: "wii" },
    // The Wii's disc magic, raw and as wrapped by a WBFS container.
    Signature { offset: 0x18, magic: &[0x5D, 0x1C, 0x9E, 0xA3], platform: "wii" },
    Signature { offset: 0x218, magic: &[0x5D, 0x1C, 0x9E, 0xA3], platform: "wii" },
    Signature { offset: 0x1C, magic: &[0xC2, 0x33, 0x9F, 0x3D], platform: "gamecube" },
    Signature { offset: 0x21C, magic: &[0xC2, 0x33, 0x9F, 0x3D], platform: "gamecube" },
    // --- Sega
    Signature { offset: 0, magic: b"SEGADISCSYSTEM", platform: "segacd" },
    Signature { offset: 0x10, magic: b"SEGADISCSYSTEM", platform: "segacd" },
    Signature { offset: 0, magic: b"SEGA SEGASATURN", platform: "saturn" },
    Signature { offset: 0x10, magic: b"SEGA SEGASATURN", platform: "saturn" },
    Signature { offset: 0, magic: b"SEGA SEGAKATANA", platform: "dreamcast" },
    Signature { offset: 0x10, magic: b"SEGA SEGAKATANA", platform: "dreamcast" },
    Signature { offset: 0x100, magic: b"SEGA MEGA DRIVE", platform: "genesis" },
    Signature { offset: 0x100, magic: b"SEGA GENESIS", platform: "genesis" },
    Signature { offset: 0x100, magic: b"SEGA 32X", platform: "sega32x" },
    // --- Sony
    Signature { offset: 0, magic: b"PS-X EXE", platform: "ps1" },
];

/// How far in the furthest signature reaches.
const HEAD_BYTES: usize = 0x300;

/// The system a file's own bytes claim, if any of them do.
pub fn identify(path: &Path) -> Option<&'static str> {
    let mut head = vec![0u8; HEAD_BYTES];
    let read = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut head))
        .ok()?;
    head.truncate(read);
    if let Some(found) = identify_bytes(&head) {
        return Some(found);
    }
    identify_iso9660(path)
}

// ------------------------------------------------------------- ISO 9660

/// Where an ISO 9660 image keeps its volume descriptor, and how big a sector
/// is in one.
const SECTOR: u64 = 2048;
const PVD_SECTOR: u64 = 16;

/// Identify a disc image by reading its filesystem.
///
/// Sony discs carry no magic number in their first bytes, which is why a
/// PlayStation 2 game came out as Unidentified: nothing in the header says
/// what it is. What does say is a file in the root of the disc. `SYSTEM.CNF`
/// names the executable to boot, under the key `BOOT2` on a PlayStation 2 and
/// `BOOT` on a PlayStation. A PSP disc instead carries `PSP_GAME` and
/// `UMD_DATA.BIN`.
///
/// So the disc's own directory is read: the volume descriptor at sector 16,
/// the root directory it points at, and then the one small file that answers
/// the question. Three short reads, wherever the image happens to be on disk.
///
/// Only images with 2048-byte sectors are understood, which is what `.iso`
/// means. A raw `.bin` track uses 2352-byte sectors with error correction
/// around each one, and is a different problem.
pub fn identify_iso9660(path: &Path) -> Option<&'static str> {
    let mut file = std::fs::File::open(path).ok()?;

    let mut pvd = [0u8; SECTOR as usize];
    file.seek(SeekFrom::Start(PVD_SECTOR * SECTOR)).ok()?;
    file.read_exact(&mut pvd).ok()?;
    // Descriptor type 1 followed by the standard identifier: a primary
    // volume descriptor, and therefore an ISO 9660 filesystem.
    if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
        return None;
    }

    // The root directory's own record is embedded at offset 156.
    let (root_lba, root_len) = extent(&pvd[156..156 + 34])?;
    if root_len == 0 || root_len > 4 * 1024 * 1024 {
        return None;
    }

    let mut dir = vec![0u8; root_len];
    file.seek(SeekFrom::Start(root_lba * SECTOR)).ok()?;
    file.read_exact(&mut dir).ok()?;
    let entries = read_directory(&dir);

    let named = |want: &str| entries.iter().any(|(name, _, _)| name == want);

    if named("PSP_GAME") || named("UMD_DATA.BIN") {
        return Some("psp");
    }

    if let Some((_, lba, len)) = entries.iter().find(|(n, _, _)| n == "SYSTEM.CNF") {
        let mut buf = vec![0u8; (*len).min(4096)];
        if file.seek(SeekFrom::Start(lba * SECTOR)).is_ok()
            && file.read_exact(&mut buf).is_ok()
        {
            let text = String::from_utf8_lossy(&buf).to_ascii_uppercase();
            // BOOT2 is the PlayStation 2 key; plain BOOT is the original.
            if text.contains("BOOT2") {
                return Some("ps2");
            }
            if text.contains("BOOT") {
                return Some("ps1");
            }
        }
    }

    None
}

/// The sector and byte length a directory record points at.
fn extent(record: &[u8]) -> Option<(u64, usize)> {
    if record.len() < 14 {
        return None;
    }
    let lba = u32::from_le_bytes(record[2..6].try_into().ok()?) as u64;
    let len = u32::from_le_bytes(record[10..14].try_into().ok()?) as usize;
    Some((lba, len))
}

/// Walk an ISO 9660 directory into (name, sector, length) triples.
fn read_directory(dir: &[u8]) -> Vec<(String, u64, usize)> {
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < dir.len() {
        let len = dir[i] as usize;
        if len == 0 {
            // Records never straddle a sector, so a zero length means the rest
            // of this sector is padding.
            let next = (i / SECTOR as usize + 1) * SECTOR as usize;
            if next <= i || next >= dir.len() {
                break;
            }
            i = next;
            continue;
        }
        if i + len > dir.len() || len < 34 {
            break;
        }
        let record = &dir[i..i + len];
        let name_len = record[32] as usize;
        if record.len() >= 33 + name_len {
            let raw = &record[33..33 + name_len];
            // Names carry a ";1" version suffix that nothing here wants.
            let name = String::from_utf8_lossy(raw)
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_uppercase();
            if let Some((lba, size)) = extent(record) {
                out.push((name, lba, size));
            }
        }
        i += len;
    }
    out
}

pub fn identify_bytes(head: &[u8]) -> Option<&'static str> {
    SIGNATURES
        .iter()
        .find(|s| {
            head.len() >= s.offset + s.magic.len()
                && &head[s.offset..s.offset + s.magic.len()] == s.magic
        })
        .map(|s| s.platform)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(offset: usize, magic: &[u8]) -> Vec<u8> {
        let mut v = vec![0u8; HEAD_BYTES];
        v[offset..offset + magic.len()].copy_from_slice(magic);
        v
    }

    /// The three read off real dumps on this machine.
    #[test]
    fn reads_the_signatures_taken_from_real_files() {
        // A Switch NSP opens with PFS0.
        assert_eq!(identify_bytes(&at(0, b"PFS0")), Some("switch"));

        // A Wii dump in a WBFS container: the container magic at 0, and the
        // disc header it wraps at 0x200, so the disc magic lands at 0x218.
        assert_eq!(identify_bytes(&at(0, b"WBFS")), Some("wii"));
        assert_eq!(
            identify_bytes(&at(0x218, &[0x5D, 0x1C, 0x9E, 0xA3])),
            Some("wii")
        );

        // And a raw disc image, where it sits at 0x18.
        assert_eq!(
            identify_bytes(&at(0x18, &[0x5D, 0x1C, 0x9E, 0xA3])),
            Some("wii")
        );
    }

    #[test]
    fn tells_gamecube_from_wii() {
        assert_eq!(
            identify_bytes(&at(0x1C, &[0xC2, 0x33, 0x9F, 0x3D])),
            Some("gamecube")
        );
        assert_eq!(
            identify_bytes(&at(0x18, &[0x5D, 0x1C, 0x9E, 0xA3])),
            Some("wii")
        );
    }

    #[test]
    fn reads_the_sega_disc_headers() {
        assert_eq!(identify_bytes(&at(0, b"SEGADISCSYSTEM")), Some("segacd"));
        assert_eq!(identify_bytes(&at(0, b"SEGA SEGASATURN")), Some("saturn"));
        assert_eq!(
            identify_bytes(&at(0x10, b"SEGA SEGAKATANA")),
            Some("dreamcast")
        );
    }

    /// Build a small but genuine ISO 9660 image: a volume descriptor at
    /// sector 16, a root directory holding one file, and that file's contents.
    fn build_iso(file_name: &str, contents: &[u8]) -> Vec<u8> {
        const SEC: usize = 2048;
        let root_sector = 17usize;
        let file_sector = 18usize;
        let mut iso = vec![0u8; SEC * 20];

        // --- one directory record for the file
        let name = file_name.as_bytes();
        let rec_len = 33 + name.len() + (1 - name.len() % 2);
        let mut rec = vec![0u8; rec_len];
        rec[0] = rec_len as u8;
        rec[2..6].copy_from_slice(&(file_sector as u32).to_le_bytes());
        rec[10..14].copy_from_slice(&(contents.len() as u32).to_le_bytes());
        rec[32] = name.len() as u8;
        rec[33..33 + name.len()].copy_from_slice(name);

        let root_off = root_sector * SEC;
        iso[root_off..root_off + rec.len()].copy_from_slice(&rec);

        // --- the primary volume descriptor
        let pvd = 16 * SEC;
        iso[pvd] = 1;
        iso[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        // The root directory's own record lives at offset 156.
        let r = pvd + 156;
        iso[r] = 34;
        iso[r + 2..r + 6].copy_from_slice(&(root_sector as u32).to_le_bytes());
        iso[r + 10..r + 14].copy_from_slice(&(SEC as u32).to_le_bytes());

        // --- the file itself
        let f = file_sector * SEC;
        iso[f..f + contents.len()].copy_from_slice(contents);
        iso
    }

    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "playdex-iso-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// A PlayStation 2 disc names its boot executable under BOOT2. Nothing in
    /// the header says "PlayStation", which is why these came out as
    /// Unidentified until the filesystem was read.
    #[test]
    fn reads_a_playstation_2_disc_out_of_its_filesystem() {
        let iso = build_iso("SYSTEM.CNF;1", b"BOOT2 = cdrom0:SLUS_203.12;1 VER=1.00");
        let path = write_temp("Sonic Riders.iso", &iso);
        assert_eq!(identify(&path), Some("ps2"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// The original PlayStation uses BOOT, without the 2.
    #[test]
    fn tells_a_playstation_1_disc_from_a_2() {
        let iso = build_iso("SYSTEM.CNF;1", b"BOOT = cdrom:SLUS_004.02;1 TCB = 4");
        let path = write_temp("Some PS1 Game.iso", &iso);
        assert_eq!(identify(&path), Some("ps1"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A PSP disc is known by what sits in its root instead.
    #[test]
    fn reads_a_psp_disc() {
        let iso = build_iso("UMD_DATA.BIN;1", b"ULUS10041|0001|G");
        let path = write_temp("Some PSP Game.iso", &iso);
        assert_eq!(identify(&path), Some("psp"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// Something that is not a disc image at all stays unidentified.
    #[test]
    fn an_iso_that_is_not_iso9660_says_nothing() {
        let path = write_temp("junk.iso", &vec![0u8; 2048 * 20]);
        assert_eq!(identify(&path), None);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn says_nothing_when_it_recognises_nothing() {
        assert_eq!(identify_bytes(&vec![0u8; HEAD_BYTES]), None);
        assert_eq!(identify_bytes(b"not a rom"), None);
        // Too short to reach an offset is not a match.
        assert_eq!(identify_bytes(b"WBF"), None);
    }
}
