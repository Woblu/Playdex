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
file gets CRC32, MD5 and SHA1 computed in a single pass. For a `.zip` or a
`.7z`, the ROM *inside* the archive is hashed rather than the archive itself,
because that is what the metadata databases and the No-Intro/Redump DATs
index — hashing the container instead would be worse than not hashing at all,
since it matches nothing but still looks like a real dump hash. The archive
header lists every entry and its size, so picking the ROM costs a header read;
only the entry we settle on is decompressed. `.rar` remains opaque.

Entries already in the library are left alone unless they were indexed under a
rule that has since been fixed — an unresolved platform, or an archive recorded
without ever being opened. Those are re-detected and re-hashed in place, and
the scan summary counts them as *corrected*. Titles, artwork and a platform you
set by hand are never overwritten.

An entry the scanner added and now recognises as a mistake is *dropped* from
the library, so a fixed heuristic cleans up after itself instead of leaving
yesterday's junk behind. The file on disk is never touched. Anything you have
played, favourited, patched or used as the base for a hack is exempt however it
now reads — a heuristic does not get to throw away something you have shown you
care about.

**Rejecting what isn't a game.** ROM folders collect manuals, box scans, BIOS
dumps and the odd installer, and extensions lie — a `.bin` is as likely to be
firmware or a CD audio track as a game. Every candidate is inspected before it
is hashed: magic numbers for images, PDFs, Office documents, executables and
media; BIOS filename patterns; and a 1 KB size floor. Headers are positively
confirmed for NES, Game Boy, GBA, N64 and Mega Drive.

Emulators get unpacked into ROM libraries constantly, and an emulator is a
folder full of things that read as ROMs — Dolphin's `Sys` directory alone
holds a dozen `.bin` files of firmware, fonts and cheat databases. So a
directory containing a program is skipped whole rather than walked: spotting
the install at its root is both cheaper and more reliable than trying to name
every file inside it. The library folders you chose yourself are never subject
to this, only directories below them. An archive is judged the same way by its
entries — one carrying an executable is an application, not a game dump.

The bias is toward keeping files. Only definite evidence rejects one; a header
mismatch is reported as *suspect* and indexed anyway, because losing a real
game is worse than listing a stray file. The scan summary says what was thrown
out and why, so a missing game is traceable.

**Platform detection.** Extensions are not always enough — `.bin`, `.cue`,
`.iso`, `.chd` and `.zip` are shared across a dozen systems. Playdex resolves
them in this order:

1. The platform assigned to the library folder, if you set one.
2. An extension only one system uses (`.sfc`, `.gba`, `.nes` …).
3. For an archive, an unambiguous extension among its entries.
4. A directory name along the path matching a system alias
   (`.../Sega Genesis/...`).
5. The ROM's own name, and the names inside the archive. Dumps are routinely
   called "Mario Kart Wii" or "Sonic (Mega Drive)", and for a container
   extension like `.7z` — which belongs to no system at all — that is the last
   real evidence there is. Matched on whole words only: the looser
   substring rule used for directory names reads "Legbreaker" as Game Boy,
   which is fine for a folder someone named after a system and useless for a
   game title.
6. Otherwise the file is filed under the first candidate system, and you can
   correct it from the game's detail panel.

Extensions that genuinely belong to two systems are left ambiguous rather than
assigned to one: Dolphin's `.rvz`, `.gcz` and `.wia` each hold either a
GameCube or a Wii disc, so they fall through to the steps above instead of
quietly filing every Wii dump under GameCube.

**Artwork with no setup.** libretro-thumbnails — RetroArch's own artwork server
— is the default source for box art, screenshots and title screens. No account,
no API key, no quota: the URL *is* the query, built from the system name and the
ROM's No-Intro filename, so it works the moment you finish a scan. It serves
images only, so the credentialed providers below still earn their place for
descriptions, developer, genre and release dates.

The system-name table in `scrape/libretro.rs` was taken from the
libretro-thumbnails repository listing rather than guessed — a wrong folder name
there is a silent 404.

**Metadata.** A scan that finds new games goes and fetches their details as
soon as it finishes — a game arriving with nothing but a filename is not a
second thing to remember. Only entries that still need it are looked up, so
nothing is re-fetched, and the progress toast can cancel it. ScreenScraper is
tried first, keyed on the ROM's hash, so a
correct dump matches exactly rather than by fuzzy name. If it has no match or
its quota is spent, TheGamesDB is tried by name — results are scored against
the cleaned-up title and boosted when the platform agrees, and a weak match is
discarded rather than guessed at. Both providers name the system they matched,
so a game whose platform we never worked out takes it from them; a platform
already on the record is left alone, since it may have been set by hand. Artwork is downloaded once into the app data
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

**Three skins.** The same library laid out for where you are using it,
chosen under **Settings -> Appearance**: the default desktop view (sidebar,
sortable grid, detail panel), a console home screen (one row of large icons
sized against the window rather than fixed, a caption beneath, and a row of
small round system buttons that each go somewhere different - including an
"All software" grid for when the row gets long), and a living-room layout (hero art,
left rail, capsule shelf). They are separate component trees, not a palette
swap, because a console home screen is a different shape rather than a
different colour. All state, fetching and side effects stay in `App`, so the
three cannot drift apart in behaviour - only in appearance. Anything a skin
does not draw itself (cheats, saves, ROM hacks) opens the shared detail panel,
so no feature is available in one skin and missing from another. Nothing is
traced from another product's assets: the shapes are the obvious ones for each
context, drawn in our own CSS.

**Controller.** A pad is picked up on its own - no pairing step. The D-pad or
left stick moves focus, A presses what is focused (or starts the game, on a
game tile), B closes, X opens a game's panel, Y favourites it, the shoulders
step through systems and Start plays the selection. Movement is by geometry
rather than a hand-written map of what sits next to what: everything
ordinarily interactive is a candidate, and the nearest thing in the direction
you pushed wins, weighted so travel along the axis beats drift across it. That
means a new button or a whole new skin is navigable the moment it renders,
with no second structure to keep in sync - including inside Settings, which
gets controller navigation for free. The focus ring thickens while a pad is
connected, since it has to be visible from a sofa.

**Stats.** Playtime is recorded per session, and the library view surfaces
total time, session count, longest session, and jump-back-in and most-played
lists that open the game.

**Cheats apply themselves.** Switching a cheat on here is the whole
interaction: pressing Play writes the enabled codes into RetroArch's cheat
folder — under the *core's* display name, which is where RetroArch actually
looks — and, if the game has cheats on, turns on RetroArch's "Auto-Apply
Cheats During Game Load", since a cheat file it never reads would do nothing.
Launch is the right moment for both: RetroArch is definitely not running, so
it cannot overwrite its own config on exit. **Write now** is there only for
prepping a ROM you intend to start from RetroArch directly.

**Archives are unpacked when they have to be.** RetroArch ships an `.info`
file beside every core listing the extensions it accepts, so Playdex reads
that rather than guessing. Dolphin's says
`gcm|iso|wbfs|ciso|gcz|elf|dol|dff|tgc|wad|rvz|m3u|wia` - no archive format at
all - so a Wii game kept as a `.7z` is unpacked once into the app data folder
and the `.wbfs` inside is handed over instead. A `.zip` of a NES ROM is left
alone, because Nestopia's list does include `zip` and RetroArch unpacks it
itself. With no info file to consult the rule falls back to unpacking `.7z`
only. Unpacked ROMs are keyed by the archive's checksum and reused, so this
costs one extraction rather than one per launch - but a disc image is several
gigabytes, and that space does get used.

**Launching.** RetroArch is invoked as `retroarch -L <core> <rom>`. Each system
has a preferred libretro core, overridable per system, or you can point a system
at a standalone emulator with a command template using `{rom}`. The detail panel
shows the exact command that will run, so a wrong path is visible before you
click Play. When the emulator exits, the session length is added to that game's
playtime.

**Updating itself.** Playdex checks for a new version a few seconds after
launch — after the library has drawn, so a slow or unreachable endpoint never
delays the app opening — and if it finds one, says so in a corner card with
the release notes. Nothing downloads until you accept: an update is news, not
an errand, and it should not stand between you and the game you opened the app
to play. A failed check is silent, because being offline is an ordinary state
for a desktop app; the **Check for updates** button under
**Settings -> Appearance -> About** reports the real reason, since a button
press deserves an answer. Every update is verified against a public key
compiled into the app before a byte of it runs, so a tampered or mis-hosted
file is refused rather than installed.

## Releasing

Updates are published as GitHub releases and found through the endpoint in
`plugins.updater.endpoints`. Pushing a version tag is the whole ritual:

```bash
# bump `version` in package.json, src-tauri/tauri.conf.json and
# src-tauri/Cargo.toml so all three agree
git commit -am "Console skin, controller navigation, in-app updates"
git tag v0.2.0
git push && git push --tags
```

`.github/workflows/release.yml` then builds the installer, signs it, and
uploads it with a `latest.json` manifest. **The commit message becomes the
release notes**, which is what the in-app notice shows — so write that commit
message for the person reading it, not for yourself.

The repository has to be **public**. The updater fetches
`releases/latest/download/latest.json` with no credentials; on a private repo
that request is a 404 and every check fails silently.

**The signing key.** `tauri signer generate` produced a keypair; the public
half is in `tauri.conf.json` and the private half is deliberately outside this
repository, in `~/.playdex/updater.key` (that folder ignores itself, so a
stray `git add -A` in a home directory cannot pick it up). Two things follow
from that:

- **Anyone holding the private key can sign an update Playdex will trust and
  install.** It belongs in a password manager and in GitHub repository
  secrets, nowhere else.
- **Losing it is unrecoverable.** Copies already installed only accept updates
  signed by the matching key; a new keypair means every existing install has
  to be replaced by hand. Back it up before you need it.

CI needs it as two secrets under *Settings -> Secrets and variables ->
Actions*: `TAURI_SIGNING_PRIVATE_KEY` (the file's contents) and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty unless you set one).

To build a signed installer locally:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.playdex/updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run tauri build
```

Without those the build still produces an installer, but stops short of the
`.sig` files and says so — an unsigned release would be one no existing copy
could install.

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

Systems with no libretro core in existence - the Switch, for one - carry an
empty core list, so launching one says to point it at a standalone emulator
rather than blaming your RetroArch setup.

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
