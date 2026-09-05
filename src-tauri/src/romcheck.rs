//! Telling ROMs apart from everything else that ends up in a ROM folder.
//!
//! Extensions lie. A `.bin` is as likely to be a BIOS, a CD audio track or a
//! firmware blob as a game, and folders collect manuals, box scans, save
//! files and the odd installer. This reads the first bytes of a file and
//! decides.
//!
//! The bias is deliberately toward keeping things: a file is only rejected on
//! definite evidence. Anything merely odd is indexed and reported as suspect,
//! because losing a real game is far worse than listing a stray file.

use std::io::Read;
use std::path::Path;

use crate::platforms;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Index it.
    Rom,
    /// Index it, but say so — the contents do not match the extension.
    Suspect(String),
    /// Definitely not a game; leave it out.
    NotRom(String),
}

/// Nothing smaller than this has ever been a game.
const MIN_ROM_BYTES: u64 = 1024;

/// Magic numbers for things that are definitely not ROMs. Zip and 7z are
/// absent on purpose — those are handled as archives elsewhere.
const NOT_ROM_MAGIC: &[(&[u8], &str)] = &[
    (b"%PDF", "a PDF"),
    (b"\x89PNG\r\n\x1a\n", "a PNG image"),
    (b"\xff\xd8\xff", "a JPEG image"),
    (b"GIF8", "a GIF image"),
    (b"RIFF", "an audio or video file"),
    (b"OggS", "an Ogg file"),
    (b"ID3", "an MP3"),
    (b"\x1f\x8b", "a gzip archive"),
    (b"Rar!\x1a\x07", "a RAR archive"),
    (b"\x7fELF", "a Linux executable"),
    (b"MZ", "a Windows executable"),
    (b"\xd0\xcf\x11\xe0", "an Office document"),
    (b"SQLite format 3", "a database"),
    (b"\x1a\x45\xdf\xa3", "a Matroska video"),
    (b"<!DO", "an HTML file"),
    (b"<htm", "an HTML file"),
    (b"<?xm", "an XML file"),
    (b"{\\rtf", "an RTF document"),
];

/// Filenames that mean firmware rather than a game. Every emulated system
/// wants one of these somewhere, and they live alongside the ROMs.
const BIOS_HINTS: &[&str] = &[
    "bios", "scph", "disksys", "syscard", "gba_bios", "gb_bios", "dmg_boot",
    "cgb_boot", "sgb_boot", "boot rom", "bootrom", "firmware", "kickstart",
    "neogeo.zip", "lynxboot", "panafz", "goldstar", "saturn_bios", "sega_101",
    "mpr-17933", "3do_bios", "pcfx", "x68000", "spc7110",
];

/// Header signatures we can positively confirm, per platform.
fn expected_magic(platform: &str) -> Option<(usize, &'static [&'static [u8]], &'static str)> {
    match platform {
        // iNES / NES 2.0 header.
        "nes" => Some((0, &[b"NES\x1a"], "NES")),
        // The Nintendo logo every cartridge must carry to boot.
        "gb" | "gbc" => Some((0x104, &[b"\xce\xed\x66\x66"], "Game Boy")),
        "gba" => Some((0x04, &[b"\x24\xff\xae\x51"], "Game Boy Advance")),
        // Three byte orders are all in circulation.
        "n64" => Some((
            0,
            &[b"\x80\x37\x12\x40", b"\x37\x80\x40\x12", b"\x40\x12\x37\x80"],
            "Nintendo 64",
        )),
        "genesis" | "sega32x" => Some((0x100, &[b"SEGA"], "Mega Drive")),
        _ => None,
    }
}

fn starts_with_at(buf: &[u8], offset: usize, needle: &[u8]) -> bool {
    buf.len() >= offset + needle.len() && &buf[offset..offset + needle.len()] == needle
}

fn looks_like_bios(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    BIOS_HINTS.iter().any(|hint| lower.contains(hint))
}

/// Decide what a file is. `platform` is what detection concluded from the
/// path, used only to confirm a header when we know what one should look like.
pub fn inspect(path: &Path, platform: &str) -> Verdict {
    let Ok(meta) = std::fs::metadata(path) else {
        return Verdict::NotRom("unreadable".into());
    };

    if meta.len() < MIN_ROM_BYTES {
        return Verdict::NotRom(format!("only {} bytes", meta.len()));
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if looks_like_bios(file_name) {
        return Verdict::NotRom("a BIOS or firmware file".into());
    }

    // 64 KiB covers every header we check, including ISO 9660 at 0x8001.
    let mut head = vec![0u8; 64 * 1024];
    let read = match std::fs::File::open(path).and_then(|mut f| f.read(&mut head)) {
        Ok(n) => n,
        Err(_) => return Verdict::NotRom("unreadable".into()),
    };
    head.truncate(read);

    for (magic, description) in NOT_ROM_MAGIC {
        if head.starts_with(magic) {
            return Verdict::NotRom(format!("{description}"));
        }
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Archives are opened elsewhere; their contents decide.
    if platforms::ARCHIVE_EXTS.contains(&ext.as_str())
        || platforms::OPAQUE_ARCHIVE_EXTS.contains(&ext.as_str())
    {
        return Verdict::Rom;
    }

    // Where a header is well defined, a mismatch is worth flagging — but not
    // worth discarding a file over. Headerless dumps do exist.
    if let Some((offset, candidates, system)) = expected_magic(platform) {
        if head.len() > offset && !candidates.iter().any(|m| starts_with_at(&head, offset, m)) {
            return Verdict::Suspect(format!("no {system} header"));
        }
    }

    Verdict::Rom
}

/// Group rejection reasons into a sentence for the scan summary.
pub fn summarise(reasons: &[String]) -> String {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for reason in reasons {
        *counts.entry(reason.as_str()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(reason, n)| format!("{n} × {reason}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("playdex-rc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Padding so files clear the size floor.
    fn padded(prefix: &[u8], len: usize) -> Vec<u8> {
        let mut v = prefix.to_vec();
        v.resize(len.max(prefix.len()), 0);
        v
    }

    #[test]
    fn accepts_a_real_nes_rom() {
        let dir = tmp("nes");
        let rom = write(&dir, "Super Mario Bros.nes", &padded(b"NES\x1a", 40_000));
        assert_eq!(inspect(&rom, "nes"), Verdict::Rom);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_box_art_and_manuals_hiding_in_the_folder() {
        let dir = tmp("junk");
        let jpg = write(&dir, "Super Mario Bros.bin", &padded(b"\xff\xd8\xff\xe0", 9000));
        let pdf = write(&dir, "manual.bin", &padded(b"%PDF-1.4", 9000));
        let exe = write(&dir, "setup.bin", &padded(b"MZ\x90\x00", 9000));

        assert!(matches!(inspect(&jpg, "nes"), Verdict::NotRom(r) if r.contains("JPEG")));
        assert!(matches!(inspect(&pdf, "nes"), Verdict::NotRom(r) if r.contains("PDF")));
        assert!(matches!(inspect(&exe, "nes"), Verdict::NotRom(r) if r.contains("Windows")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_bios_files_by_name() {
        let dir = tmp("bios");
        for name in ["scph1001.bin", "gba_bios.bin", "disksys.rom", "BIOS.bin"] {
            let f = write(&dir, name, &padded(b"\x00", 200_000));
            assert!(
                matches!(inspect(&f, "ps1"), Verdict::NotRom(r) if r.contains("BIOS")),
                "{name} should be rejected"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_files_too_small_to_be_a_game() {
        let dir = tmp("tiny");
        let f = write(&dir, "notes.nes", b"hello");
        assert!(matches!(inspect(&f, "nes"), Verdict::NotRom(r) if r.contains("bytes")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A headerless dump is odd, not disqualifying — it gets indexed anyway.
    #[test]
    fn flags_a_header_mismatch_without_discarding_it() {
        let dir = tmp("suspect");
        let f = write(&dir, "mystery.nes", &padded(b"\x00\x01\x02\x03", 40_000));
        assert!(matches!(inspect(&f, "nes"), Verdict::Suspect(r) if r.contains("NES")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn accepts_every_n64_byte_order() {
        let dir = tmp("n64");
        for (i, magic) in [
            b"\x80\x37\x12\x40".as_slice(),
            b"\x37\x80\x40\x12".as_slice(),
            b"\x40\x12\x37\x80".as_slice(),
        ]
        .iter()
        .enumerate()
        {
            let f = write(&dir, &format!("game{i}.z64"), &padded(magic, 100_000));
            assert_eq!(inspect(&f, "n64"), Verdict::Rom, "byte order {i}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Archives are judged by what is inside them, not their own bytes.
    #[test]
    fn leaves_archives_alone() {
        let dir = tmp("zip");
        let f = write(&dir, "game.zip", &padded(b"PK\x03\x04", 9000));
        assert_eq!(inspect(&f, "nes"), Verdict::Rom);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn summarises_reasons_by_count() {
        let out = summarise(&[
            "a JPEG image".into(),
            "a JPEG image".into(),
            "a BIOS or firmware file".into(),
        ]);
        assert!(out.contains("2 × a JPEG image"));
        assert!(out.contains("1 × a BIOS or firmware file"));
    }
}
