//! Static platform table: file extensions, directory-name aliases used to
//! disambiguate shared extensions, and preferred libretro cores.

pub struct PlatformDef {
    pub slug: &'static str,
    pub name: &'static str,
    pub exts: &'static [&'static str],
    /// Lowercase tokens matched against parent directory names and against
    /// provider platform names.
    pub aliases: &'static [&'static str],
    /// Preferred libretro cores, best first (filename without extension).
    pub cores: &'static [&'static str],
}

pub const PLATFORMS: &[PlatformDef] = &[
    PlatformDef {
        slug: "nes",
        name: "Nintendo Entertainment System",
        exts: &["nes", "unf", "unif", "fds"],
        aliases: &["nes", "famicom", "nintendo entertainment system", "fds"],
        cores: &["mesen_libretro", "nestopia_libretro", "fceumm_libretro"],
    },
    PlatformDef {
        slug: "snes",
        name: "Super Nintendo",
        exts: &["sfc", "smc", "swc", "fig", "bs"],
        aliases: &["snes", "super nintendo", "super famicom", "sfc"],
        cores: &["snes9x_libretro", "bsnes_libretro", "mesen-s_libretro"],
    },
    PlatformDef {
        slug: "n64",
        name: "Nintendo 64",
        exts: &["n64", "z64", "v64", "ndd"],
        aliases: &["n64", "nintendo 64"],
        cores: &["mupen64plus_next_libretro", "parallel_n64_libretro"],
    },
    PlatformDef {
        slug: "gamecube",
        name: "Nintendo GameCube",
        exts: &["gcm", "gcz", "rvz"],
        aliases: &["gamecube", "ngc", "gcn"],
        cores: &["dolphin_libretro"],
    },
    PlatformDef {
        slug: "wii",
        name: "Nintendo Wii",
        exts: &["wbfs", "wad"],
        aliases: &["wii"],
        cores: &["dolphin_libretro"],
    },
    PlatformDef {
        slug: "gb",
        name: "Game Boy",
        exts: &["gb"],
        aliases: &["gb", "game boy", "gameboy"],
        cores: &["sameboy_libretro", "gambatte_libretro"],
    },
    PlatformDef {
        slug: "gbc",
        name: "Game Boy Color",
        exts: &["gbc"],
        aliases: &["gbc", "game boy color", "gameboy color"],
        cores: &["sameboy_libretro", "gambatte_libretro"],
    },
    PlatformDef {
        slug: "gba",
        name: "Game Boy Advance",
        exts: &["gba"],
        aliases: &["gba", "game boy advance", "gameboy advance"],
        cores: &["mgba_libretro", "vbam_libretro"],
    },
    PlatformDef {
        slug: "nds",
        name: "Nintendo DS",
        exts: &["nds", "dsi"],
        aliases: &["nds", "nintendo ds"],
        cores: &["melonds_libretro", "desmume_libretro"],
    },
    PlatformDef {
        slug: "n3ds",
        name: "Nintendo 3DS",
        exts: &["3ds", "cci", "cxi", "cia"],
        aliases: &["3ds", "nintendo 3ds"],
        cores: &["citra_libretro"],
    },
    PlatformDef {
        slug: "virtualboy",
        name: "Virtual Boy",
        exts: &["vb", "vboy"],
        aliases: &["virtual boy", "virtualboy"],
        cores: &["mednafen_vb_libretro"],
    },
    PlatformDef {
        slug: "genesis",
        name: "Sega Genesis / Mega Drive",
        exts: &["md", "gen", "smd"],
        aliases: &["genesis", "mega drive", "megadrive", "sega genesis"],
        cores: &["genesis_plus_gx_libretro", "picodrive_libretro"],
    },
    PlatformDef {
        slug: "sms",
        name: "Sega Master System",
        exts: &["sms"],
        aliases: &["master system", "sms", "sega master system"],
        cores: &["genesis_plus_gx_libretro", "picodrive_libretro"],
    },
    PlatformDef {
        slug: "gamegear",
        name: "Sega Game Gear",
        exts: &["gg"],
        aliases: &["game gear", "gamegear"],
        cores: &["genesis_plus_gx_libretro"],
    },
    PlatformDef {
        slug: "sega32x",
        name: "Sega 32X",
        exts: &["32x"],
        aliases: &["32x", "sega 32x"],
        cores: &["picodrive_libretro"],
    },
    PlatformDef {
        slug: "segacd",
        name: "Sega CD / Mega CD",
        exts: &[],
        aliases: &["sega cd", "segacd", "mega cd", "megacd"],
        cores: &["genesis_plus_gx_libretro", "picodrive_libretro"],
    },
    PlatformDef {
        slug: "saturn",
        name: "Sega Saturn",
        exts: &[],
        aliases: &["saturn", "sega saturn"],
        cores: &["mednafen_saturn_libretro", "kronos_libretro"],
    },
    PlatformDef {
        slug: "dreamcast",
        name: "Sega Dreamcast",
        exts: &["gdi", "cdi"],
        aliases: &["dreamcast", "sega dreamcast"],
        cores: &["flycast_libretro"],
    },
    PlatformDef {
        slug: "ps1",
        name: "PlayStation",
        exts: &["pbp", "ecm"],
        aliases: &["ps1", "psx", "playstation", "psone"],
        cores: &[
            "swanstation_libretro",
            "beetle_psx_hw_libretro",
            "pcsx_rearmed_libretro",
        ],
    },
    PlatformDef {
        slug: "ps2",
        name: "PlayStation 2",
        exts: &["cso"],
        aliases: &["ps2", "playstation 2"],
        cores: &["pcsx2_libretro"],
    },
    PlatformDef {
        slug: "psp",
        name: "PlayStation Portable",
        exts: &[],
        aliases: &["psp", "playstation portable"],
        cores: &["ppsspp_libretro"],
    },
    PlatformDef {
        slug: "atari2600",
        name: "Atari 2600",
        exts: &["a26"],
        aliases: &["atari 2600", "atari2600", "2600"],
        cores: &["stella_libretro"],
    },
    PlatformDef {
        slug: "atari7800",
        name: "Atari 7800",
        exts: &["a78"],
        aliases: &["atari 7800", "atari7800", "7800"],
        cores: &["prosystem_libretro"],
    },
    PlatformDef {
        slug: "lynx",
        name: "Atari Lynx",
        exts: &["lnx"],
        aliases: &["lynx", "atari lynx"],
        cores: &["handy_libretro", "mednafen_lynx_libretro"],
    },
    PlatformDef {
        slug: "jaguar",
        name: "Atari Jaguar",
        exts: &["jag", "j64"],
        aliases: &["jaguar", "atari jaguar"],
        cores: &["virtualjaguar_libretro"],
    },
    PlatformDef {
        slug: "pcengine",
        name: "PC Engine / TurboGrafx-16",
        exts: &["pce", "sgx"],
        aliases: &["pc engine", "pcengine", "turbografx", "tg16"],
        cores: &["mednafen_pce_libretro", "mednafen_supergrafx_libretro"],
    },
    PlatformDef {
        slug: "neogeo",
        name: "Neo Geo",
        exts: &["neo"],
        aliases: &["neo geo", "neogeo"],
        cores: &["fbneo_libretro", "mame_libretro"],
    },
    PlatformDef {
        slug: "ngp",
        name: "Neo Geo Pocket",
        exts: &["ngp", "ngpc"],
        aliases: &["neo geo pocket", "ngp", "neogeo pocket"],
        cores: &["mednafen_ngp_libretro"],
    },
    PlatformDef {
        slug: "wonderswan",
        name: "WonderSwan",
        exts: &["ws", "wsc"],
        aliases: &["wonderswan"],
        cores: &["mednafen_wswan_libretro"],
    },
    PlatformDef {
        slug: "threedo",
        name: "3DO",
        exts: &[],
        aliases: &["3do", "panasonic 3do"],
        cores: &["opera_libretro"],
    },
    PlatformDef {
        slug: "msx",
        name: "MSX",
        exts: &["mx1", "mx2"],
        aliases: &["msx", "msx2"],
        cores: &["bluemsx_libretro", "fmsx_libretro"],
    },
    PlatformDef {
        slug: "c64",
        name: "Commodore 64",
        exts: &["d64", "t64", "prg", "crt", "tap"],
        aliases: &["c64", "commodore 64"],
        cores: &["vice_x64_libretro"],
    },
    PlatformDef {
        slug: "amiga",
        name: "Commodore Amiga",
        exts: &["adf", "ipf", "hdf", "lha"],
        aliases: &["amiga", "commodore amiga"],
        cores: &["puae_libretro", "uae4arm_libretro"],
    },
    PlatformDef {
        slug: "colecovision",
        name: "ColecoVision",
        exts: &["col"],
        aliases: &["colecovision", "coleco"],
        cores: &["gearcoleco_libretro", "bluemsx_libretro"],
    },
    PlatformDef {
        slug: "intellivision",
        name: "Intellivision",
        exts: &["int", "itv"],
        aliases: &["intellivision"],
        cores: &["freeintv_libretro"],
    },
    PlatformDef {
        slug: "dos",
        name: "MS-DOS",
        exts: &[],
        aliases: &["dos", "ms-dos", "msdos", "dosbox"],
        cores: &["dosbox_pure_libretro"],
    },
    PlatformDef {
        slug: "scummvm",
        name: "ScummVM",
        exts: &["scummvm", "svm"],
        aliases: &["scummvm", "scumm"],
        cores: &["scummvm_libretro"],
    },
    PlatformDef {
        slug: "arcade",
        name: "Arcade",
        exts: &[],
        aliases: &["arcade", "mame", "fbneo", "fba", "final burn"],
        cores: &["fbneo_libretro", "mame_libretro"],
    },
];

/// Extensions that several platforms share. These are never enough on their own
/// to identify a platform, so they get resolved by folder assignment or by a
/// parent directory name instead.
pub const AMBIGUOUS_EXTS: &[&str] = &[
    "cue", "bin", "iso", "chd", "img", "ccd", "mds", "m3u", "toc", "nrg", "rom", "dsk",
];

/// Archives we can look inside to find the real ROM.
pub const ARCHIVE_EXTS: &[&str] = &["zip"];

/// Archives we index but cannot read into (no bundled extractor).
pub const OPAQUE_ARCHIVE_EXTS: &[&str] = &["7z", "rar"];

pub fn by_slug(slug: &str) -> Option<&'static PlatformDef> {
    PLATFORMS.iter().find(|p| p.slug == slug)
}

pub fn display_name(slug: &str) -> String {
    by_slug(slug).map(|p| p.name.to_string()).unwrap_or_else(|| {
        if slug == "unknown" {
            "Unidentified".into()
        } else {
            slug.into()
        }
    })
}

/// Platforms that claim this extension.
pub fn candidates_for_ext(ext: &str) -> Vec<&'static PlatformDef> {
    let ext = ext.to_ascii_lowercase();
    PLATFORMS
        .iter()
        .filter(|p| p.exts.contains(&ext.as_str()))
        .collect()
}

/// Match a directory name (or any free text) against platform aliases.
/// The longest alias wins, so "super nintendo" beats "nintendo".
pub fn match_alias(text: &str) -> Option<&'static PlatformDef> {
    let hay = text.trim().to_ascii_lowercase();
    let words: Vec<&str> = hay
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    let mut best: Option<(&'static PlatformDef, usize)> = None;
    for p in PLATFORMS {
        for alias in p.aliases {
            let a = alias.to_ascii_lowercase();
            let hit = hay == a || hay.contains(&a) || words.iter().any(|w| *w == a);
            if hit && best.map_or(true, |(_, len)| a.len() > len) {
                best = Some((p, a.len()));
            }
        }
    }
    best.map(|(p, _)| p)
}

/// True if we should even consider indexing this file.
pub fn is_indexable_ext(ext: &str) -> bool {
    let e = ext.to_ascii_lowercase();
    AMBIGUOUS_EXTS.contains(&e.as_str())
        || ARCHIVE_EXTS.contains(&e.as_str())
        || OPAQUE_ARCHIVE_EXTS.contains(&e.as_str())
        || PLATFORMS.iter().any(|p| p.exts.contains(&e.as_str()))
}

/// True if this extension alone identifies exactly one platform.
pub fn is_unique_ext(ext: &str) -> bool {
    candidates_for_ext(ext).len() == 1
}
