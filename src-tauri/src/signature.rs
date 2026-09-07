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

use std::io::Read;
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
    identify_bytes(&head)
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

    #[test]
    fn says_nothing_when_it_recognises_nothing() {
        assert_eq!(identify_bytes(&vec![0u8; HEAD_BYTES]), None);
        assert_eq!(identify_bytes(b"not a rom"), None);
        // Too short to reach an offset is not a match.
        assert_eq!(identify_bytes(b"WBF"), None);
    }
}
