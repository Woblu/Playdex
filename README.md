# Playdex

A ROM library manager and emulator launcher. Point it at the folders where you
keep your ROMs, and it identifies each one, fetches cover art and details for
it, and launches it through RetroArch or a standalone emulator — with playtime
tracked per game.

Built with Tauri 2 (Rust) + React + TypeScript. SQLite for the library.

## Scope

Playdex indexes ROMs **already on your disk**. It does not search for or
download games, and it is not built to. The metadata providers it talks to
(ScreenScraper, TheGamesDB) are community *metadata* databases: they serve
artwork, descriptions and release details for games they can identify. They are
not game sources.

What it *does* support is ROM hacks, which is a different thing: a patch holds
only the hack author's own changes, and the base ROM comes from you. See
**ROM hacks** below.

There is a "browse and get something new" flow, but it is for **homebrew**:
games written by hobbyists and published freely by their authors. See
**Homebrew browser** below.

## How it works

**Scanning.** Every library folder is walked for known ROM extensions. Each
file gets CRC32, MD5 and SHA1 computed in a single pass. For a `.zip`, the ROM
*inside* the archive is hashed rather than the archive itself, because that is
what the metadata databases and the No-Intro/Redump DATs index.

**Rejecting what isn't a game.** ROM folders collect manuals, box scans, BIOS
dumps and the odd installer, and extensions lie — a `.bin` is as likely to be
firmware or a CD audio track as a game. Every candidate is inspected before it
is hashed: magic numbers for images, PDFs, Office documents, executables and
media; BIOS filename patterns; and a 1 KB size floor. Headers are positively
confirmed for NES, Game Boy, GBA, N64 and Mega Drive.

The bias is toward keeping files. Only definite evidence rejects one; a header
mismatch is reported as *suspect* and indexed anyway, because losing a real
game is worse than listing a stray file. The scan summary says what was thrown
out and why, so a missing game is traceable.

**Platform detection.** Extensions are not always enough — `.bin`, `.cue`,
`.iso`, `.chd` and `.zip` are shared across a dozen systems. Playdex resolves
them in this order:

1. The platform assigned to the library folder, if you set one.
2. An extension only one system uses (`.sfc`, `.gba`, `.nes` …).
3. For a zip, an unambiguous extension among its entries.
4. A directory name along the path matching a system alias
   (`.../Sega Genesis/...`).
5. Otherwise the file is filed under the first candidate system, and you can
   correct it from the game's detail panel.

**Artwork with no setup.** libretro-thumbnails — RetroArch's own artwork server
— is the default source for box art, screenshots and title screens. No account,
no API key, no quota: the URL *is* the query, built from the system name and the
ROM's No-Intro filename, so it works the moment you finish a scan. It serves
images only, so the credentialed providers below still earn their place for
descriptions, developer, genre and release dates.

The system-name table in `scrape/libretro.rs` was taken from the
libretro-thumbnails repository listing rather than guessed — a wrong folder name
there is a silent 404.

**Metadata.** ScreenScraper is tried first, keyed on the ROM's hash, so a
correct dump matches exactly rather than by fuzzy name. If it has no match or
its quota is spent, TheGamesDB is tried by name — results are scored against
the cleaned-up title and boosted when the platform agrees, and a weak match is
discarded rather than guessed at. Artwork is downloaded once into the app data
directory and served to the UI over a private `media://` protocol scoped to
that directory.

**ROM hacks.** Point Playdex at an IPS, UPS or BPS patch and it applies it to
a *copy* of a ROM in your library — the original is never modified — then adds
the result as its own game with its own title, art, saves and playtime. UPS and
BPS embed the CRC32 of the ROM they were built against, and Playdex already
hashes every ROM during scanning, so a mismatch is reported before patching
("this patch expects CRC32 X, yours is Y — most likely a different revision or
region") instead of silently producing a corrupt game. The patch formats are
implemented in `patch.rs` and unit-tested.

**Hack catalog.** Patches can be imported in bulk — point Playdex at a folder
of them, or straight at a `.7z` bundle from a community archive. A bundle is
streamed and only the patch files inside are extracted; documentation, source
code and everything else is skipped rather than unpacked, so a 35 GB archive
costs a read instead of 35 GB of disk. Archive entry paths are refused if they
try to escape the destination directory. Each patch is indexed by the CRC32 of
the ROM it targets, and by the system and ROM name inferred from its folders.
A game's detail panel then lists exactly which catalogued hacks its dump can
run — matched on checksum, so revisions and regions are never guessed at.

**Homebrew browser.** Search the Internet Archive from inside the app and pull
a game straight into the library — downloaded, hashed, given a platform, and
decorated with the screenshot and description from its archive page, so it
arrives complete without troubling a scraper.

The search is scoped to an allowlist of two collections (`spahomebrew`,
`doshaven-homebrew`, ~294 items). That is deliberately small. The Internet
Archive hosts emulated *commercial* libraries — Console Living Room, the MS-DOS
software library — which are unreachable from here by design, and several
collections named "homebrew" turned out not to be:

| Collection | Items | Why it was rejected |
|---|---|---|
| `psp-homebrew-library` | 3,950 | Fan ports of commercial games; includes Sony's PSP BIOS |
| `the-homebrew-cloud` | 119 | Switch custom firmware and piracy tooling |
| `atari_7800_homebrew` | 275 | Almost entirely "(Hack)" entries — derivative works on commercial ROMs |
| `ps2-homebrew-library` | 238 | Loaders and cheat devices, not games |
| `psx-homebrew-library` | 512 | Genuine Net Yaroze homebrew mixed with a BIOS dumper |

Of every collection checked, only `spahomebrew` carries licence metadata (118
of 188 items are CC-licensed). Each result shows its licence where it declares
one. Even inside the allowlist, homebrew scenes produce fan remakes that borrow
commercial names, so treat the licence column as the signal. **Adding a
collection to that allowlist is a licensing decision, not a convenience one.**

**Saves.** A game's save files and save states are listed together, with
RetroArch's state thumbnails where it wrote them, found across the configured
save directories, the per-core subfolders RetroArch makes when sorting is on,
and the ROM's own folder. Back up copies everything into a dated folder. Only
save states can be deleted — battery saves hold real progress and are refused.

**Stats.** Playtime is recorded per session, and the library view surfaces
total time, session count, longest session, and jump-back-in and most-played
lists that open the game.

**Launching.** RetroArch is invoked as `retroarch -L <core> <rom>`. Each system
has a preferred libretro core, overridable per system, or you can point a system
at a standalone emulator with a command template using `{rom}`. The detail panel
shows the exact command that will run, so a wrong path is visible before you
click Play. When the emulator exits, the session length is added to that game's
playtime.

## Setup

```bash
npm install
npm run tauri dev      # development
npm run tauri build    # installer / bundled app
```

Requires Rust and, on Windows, the MSVC build tools plus the WebView2 runtime.

### Credentials

Both providers are **optional**. Artwork works with no credentials at all via
libretro-thumbnails; these two add the text metadata it cannot supply. Enter
them under **Settings → Metadata**; they are stored in the local SQLite
database.

**ScreenScraper** (`screenscraper.fr`) — needs two things:

- A *developer* ID and password, requested from the site once you have an
  account. Nothing is bundled with this app; without a dev key the API answers
  only a trickle of requests.
- Your *personal* ScreenScraper login, which raises the per-minute and daily
  quota. Contributors get a higher allowance.

**TheGamesDB** (`thegamesdb.net`) — a free API key with a monthly allowance.
Used as the fallback.

**Test credentials** in that tab saves what you have entered and asks both
providers whether the keys work, reporting your remaining allowance — so a bad
key surfaces immediately rather than as a wall of failures mid-scrape.

Scraping paces itself at roughly three requests a second and stops cleanly when
a provider reports its quota is exhausted, telling you how far it got.

### Emulators

**Settings → Emulators** tries to find RetroArch for you — it checks the
standard install folders, Steam (located via the registry, then following
`libraryfolders.vdf` across every drive — Steam is often not on C:), every drive
root, Scoop, Chocolatey, the macOS app bundle, Linux package and Flatpak paths,
and finally `PATH`. Detection runs automatically the first time you open the tab
with nothing set, and **Detect RetroArch** re-runs it. It reports where it found
the install and how many cores are present, so a wrong hit is obvious. The cores
folder is inferred alongside the executable if you leave it blank. Paths are
stripped of the surrounding quotes Explorer's "Copy as path" adds, both on save
and on use. Per-system, you can override the core or switch to a standalone
emulator:

```
"C:\Emulators\Dolphin\Dolphin.exe" -b -e "{rom}"
```

## Layout

```
src-tauri/src/
  lib.rs          app setup, media:// protocol, command registration
  commands.rs     every command the UI can call
  db.rs           SQLite schema and queries
  scan.rs         folder walking, platform detection, title cleanup
  hashing.rs      one-pass CRC32/MD5/SHA1, reads into zips
  platforms.rs    system table: extensions, aliases, preferred cores
  scrape/
    mod.rs        provider orchestration and fallback
    libretro.rs   artwork, no credentials needed
    screenscraper.rs
    thegamesdb.rs
  patch.rs        IPS/UPS/BPS patching with CRC verification
  hacks.rs        patch catalog import (folders and 7z bundles)
  homebrew.rs     Internet Archive homebrew search and install
  detect.rs       finding an installed RetroArch
  romcheck.rs     telling ROMs from manuals, BIOS dumps and box art
  cheats.rs       Game Genie codes and RetroArch's cheat format
  saves.rs        save files and save states
  media.rs        artwork download and cache
  launch.rs       command construction, process spawn, playtime
src/
  App.tsx         state, event wiring
  components/     sidebar, top bar, grid, detail panel, settings, toast
```

## Where hacks come from

ROMhacking.net went read-only in 2024 and released its database and file
archive to the Internet Archive, where it lives as the *ROM Hack Patch
Archive*: 43 files, ~35 GB, packaged as per-system 7z bundles rather than a
queryable catalog — which is why importing is a one-time local operation
instead of a live API. The active successor community is romhack.ing.

Patches only ever carry the hack author's own changes. The base ROM comes from
you.

## Ideas

- **More homebrew sources** — the current allowlist is small because most
  Internet Archive "homebrew" collections did not survive inspection. Scene
  sites that publish their own catalogues would widen it honestly. Note itch.io
  is not viable: its API is built for OAuth and purchase verification, with no
  public browse endpoint.
- **DAT matching** — verify dumps against No-Intro/Redump DATs and flag bad
  dumps, renames and duplicates.
- **Controller navigation** — the library is mouse-driven; a gamepad-friendly
  mode would suit couch play.
- **M3U grouping** — collapse multi-disc games into one entry.
- **Save state and screenshot browsing** per game.
- **Controller-friendly big-picture mode.**

## Notes

- Removing a game removes the library entry only; the file on disk is never
  touched. Nothing in this app deletes ROMs.
- The library lives in the app data directory alongside the artwork cache, so
  deleting that folder resets everything without touching your ROMs.
