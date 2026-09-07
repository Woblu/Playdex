# Playdex

A ROM library manager and emulator launcher. Point it at the folders where you
keep your ROMs. It identifies each one, pulls down cover art and details, and
launches it through RetroArch or a standalone emulator. Playtime is tracked per
game.

Tauri 2 (Rust) + React + TypeScript, with SQLite for the library.

## Scope

Playdex indexes ROMs you already have on disk. It doesn't search for or
download games. ScreenScraper and TheGamesDB are metadata databases: they serve
artwork and descriptions for games they can identify, not the games themselves.

ROM hacks are a different thing and are supported. A patch contains only the
hack author's own changes; the base ROM comes from you.

## What it does

### Scanning

Every library folder is walked for known ROM extensions. Each file gets CRC32,
MD5 and SHA1 in a single pass.

For a `.zip` or `.7z`, the ROM *inside* is hashed, not the archive. That's what
the metadata databases and the No-Intro/Redump DATs index, so hashing the
container matches nothing while still looking like a real dump hash. Archive
headers list every entry and its size, so choosing the right one costs a header
read and only that entry gets decompressed. `.rar` stays opaque.

Existing entries are normally left alone. Two exceptions:

- An entry indexed under a rule that's since been fixed (unresolved platform,
  or an archive that was never opened) gets re-detected and re-hashed in place.
  The scan summary calls these *corrected*. Titles, artwork and any platform
  you set by hand survive.
- An entry the scanner now recognises as a mistake gets *dropped*, so a fixed
  heuristic cleans up after itself. The file on disk is never touched. Anything
  you've played, favourited, patched or used as a hack's base game is exempt.

### Adding games

Drag a ROM onto the window and it joins the library. The whole window is the
target, since someone dragging a file at a program is not aiming at anything in
particular.

Dropped files are indexed where they lie rather than copied anywhere. It is
your ROM in your folder, and quietly duplicating gigabytes into the app data
directory to make the bookkeeping tidier is not a trade worth making. Drop a
*folder* and it becomes a library folder, then gets scanned, which is plainly
what dropping a folder means.

Either way it runs the same indexing a scan does: same platform detection, same
rejection rules, same hashing, same cleaned-up title, and the same automatic
metadata fetch afterwards. A second "quick add" path would be a second place
for those rules to drift.

### Telling ROMs from everything else

ROM folders fill up with manuals, box scans, BIOS dumps and installers, and
extensions lie. A `.bin` is as likely to be firmware or a CD audio track as a
game. Every candidate is checked before it's hashed:

- Magic numbers for images, PDFs, Office documents, executables and media
- BIOS and firmware filename patterns
- A 1 KB size floor
- Positive header confirmation for NES, Game Boy, GBA, N64 and Mega Drive

Emulators end up inside ROM libraries all the time, and an emulator is a folder
full of files that look like ROMs. Dolphin's `Sys` directory alone holds a dozen
`.bin` files of firmware, fonts and cheat databases. So any directory containing
a program is skipped whole rather than walked. Your own library folders are
never skipped, only directories underneath them. Archives get the same
treatment: one containing an executable is an application, not a game dump.

The bias is toward keeping files. Only definite evidence rejects one. A header
mismatch is flagged *suspect* and indexed anyway, since losing a real game is
worse than listing a stray file. The scan summary says what was thrown out and
why.

### Platform detection

`.bin`, `.cue`, `.iso`, `.chd` and `.zip` are shared across a dozen systems, so
extensions alone don't settle it. Playdex works through this order:

1. The platform assigned to the library folder, if you set one
2. An extension only one system uses (`.sfc`, `.gba`, `.nes`)
3. For an archive, an unambiguous extension among its entries
4. **The file's own bytes** — a magic number in its first kilobyte
5. A directory name matching a system alias (`.../Sega Genesis/...`)
6. The ROM's own filename, and the names inside the archive
7. Failing all that, the first candidate system, which you can correct from the
   game's detail panel

Step 4 is the only one that isn't a guess. Extensions and filenames are what
somebody typed; a magic number is what the machine wrote. It exists for the
ambiguous cases: a `.iso` could be six different systems, but a disc image
announces which one it is in its header. A Wii dump carries `5D1C9EA3` at
offset 0x18, or at 0x218 when a WBFS container wraps it; a GameCube disc has
`C2339F3D` at 0x1C; a Switch NSP opens with `PFS0`; Saturn, Dreamcast and Mega
CD each name themselves in plain text. Only the first kilobyte is read.

Because it runs after the extension has had its say, a signature can only
settle a file that was going to be guessed at anyway — it can never overrule a
`.sfc`. Archives skip it, since reaching a header inside one means
decompressing it and their inner extension has already spoken at step 3.

Step 5 matters more than it sounds. Dumps are routinely named "Mario Kart Wii"
or "Sonic (Mega Drive)", and for a container extension like `.7z` that belongs
to no system at all, the filename is the last real evidence available. It
matches whole words only. The looser substring rule used for directory names
reads "Legbreaker" as Game Boy, which is fine for a folder someone named after
a system and useless for a game title.

Extensions that genuinely belong to two systems stay ambiguous. Dolphin's
`.rvz`, `.gcz` and `.wia` each hold either a GameCube or a Wii disc, so they
fall through to the steps above instead of filing every Wii dump under
GameCube.

### Artwork and metadata

libretro-thumbnails (RetroArch's own artwork server) is the default source for
box art, screenshots and title screens. No account, no API key, no quota. The
URL is the query, built from the system name and the ROM's No-Intro filename,
so it works as soon as a scan finishes. It serves images only, which is why the
credentialed providers below still earn their keep.

The system-name table in `scrape/libretro.rs` was copied from the
libretro-thumbnails repository listing rather than guessed. A wrong folder name
there is a silent 404.

Finish a scan and metadata fetches automatically for anything new. Only entries
that still need it get looked up, and the progress toast can cancel it.

ScreenScraper goes first, keyed on the ROM's hash, so a correct dump matches
exactly instead of by fuzzy name. If it has no match or its quota is spent,
TheGamesDB is tried by name: results are scored against the cleaned-up title,
boosted when the platform agrees, and a weak match is discarded rather than
guessed at. Both providers report which system they matched, which fills in a
platform Playdex couldn't work out on its own. A platform already on the record
is left alone since you may have set it by hand.

Artwork downloads once into the app data directory and is served to the UI over
a private `media://` protocol scoped to that directory.

### ROM hacks

Point Playdex at an IPS, UPS or BPS patch and it patches a *copy* of a ROM in
your library. The original is never modified. The result is added as its own
game with its own title, art, saves and playtime.

UPS and BPS embed the CRC32 of the ROM they were built against, and Playdex
already hashes everything during scanning, so a mismatch is caught before
patching rather than producing a corrupt game:

```
this patch expects CRC32 X, yours is Y
most likely a different revision or region
```

The patch formats live in `patch.rs` and are unit-tested.

Patches can also be imported in bulk, either from a folder or straight from a
`.7z` bundle. Bundles are streamed and only the patch files are extracted, so a
35 GB archive costs a read instead of 35 GB of disk. Entry paths that try to
escape the destination are refused. Each patch is indexed by the CRC32 of the
ROM it targets, so a game's detail panel can list exactly which catalogued hacks
its dump will run.

### Launching

RetroArch is invoked as `retroarch -L <core> <rom>`. Each system has a preferred
libretro core which you can override, or you can point a system at a standalone
emulator using a command template with `{rom}`. The detail panel shows the exact
command before you click Play, so a wrong path is obvious. When the emulator
exits, the session length is added to that game's playtime.

Archives get unpacked first, but only when they have to be. RetroArch ships an
`.info` file next to every core listing the extensions it accepts, and no core
lists `zip` or `7z`. RetroArch handles archives itself in the frontend and never
passes one to a core, so that list says nothing about archives at all.

What it does say is whether the core loads discs, and disc cores are exactly the
ones RetroArch will not unpack for. Dolphin's list reads:

```
gcm|iso|wbfs|ciso|gcz|elf|dol|dff|tgc|wad|rvz|m3u|wia
```

Those are disc formats, so Dolphin wants a path to a real `.wbfs` and a Wii game
kept as a `.7z` gets unpacked once. A NES or N64 ROM in a `.zip` is left alone,
because RetroArch opens it itself. With no core to ask (a standalone emulator, or
a RetroArch that cannot be found) the ROM is unpacked, since a real file always
works.

Unpacked ROMs are keyed by the archive's checksum and reused, so this costs one
extraction rather than one per launch. **Settings → Emulators** shows what the
cache is using, sets its size limit (64 GB by default, since one disc game can
be most of a small limit on its own) and can empty it. Past the limit the game
you have not played in longest is dropped. The originals are never touched, and
anything cleared is rebuilt on the next launch.

Keeping both copies is the price of storing a disc game compressed, so a game's
detail panel offers to collapse them: **Unpack and keep only the ROM** moves the
unpacked file out of the cache into the folder the archive lived in, repoints
the library entry, and then deletes the archive. That order is the design — the
replacement is in place and checked before anything is removed, and moving it
out of the cache puts it beyond the reach of eviction, which would otherwise be
free to delete the only remaining copy. It is the one thing in Playdex that
deletes a file of yours, and it only happens when asked for directly.

### Cheats

Switching a cheat on is the whole interaction. Pressing Play writes the enabled
codes into RetroArch's cheat folder, under the *core's* display name, which is
where RetroArch actually looks. If the game has cheats enabled it also turns on
RetroArch's "Auto-Apply Cheats During Game Load", since a cheat file RetroArch
never reads does nothing.

Launch is the right moment for both, because RetroArch definitely isn't running
and can't overwrite its own config on exit. **Write now** exists only for
prepping a ROM you plan to start from RetroArch directly.

### Saves

Save files and save states are listed together, with RetroArch's state
thumbnails where it wrote them. Playdex looks in the configured save
directories, the per-core subfolders RetroArch creates when sorting is on, and
the ROM's own folder. Back up copies everything into a dated folder. Only save
states can be deleted; battery saves hold real progress and are refused.

### Skins

Three layouts, under **Settings → Appearance**:

- **Playdex**: the desktop view. Sidebar, sortable grid, detail panel.
- **Console**: a home screen. One row of large icons sized against the window,
  a caption underneath, and a row of small round system buttons that each go
  somewhere different, including an "All software" grid.
- **Big Picture**: living room. Hero art, left rail, capsule shelf.

They're separate component trees, not a palette swap, since a console home
screen is a different shape rather than a different colour. All state and
fetching stays in `App`, so they can't drift apart in behaviour. Anything a skin
doesn't draw itself (cheats, saves, ROM hacks) opens the shared detail panel, so
no feature exists in one skin and not another.

Nothing is traced from another product's assets. The shapes are the obvious ones
for each context, drawn in CSS here.

### Controller

A pad is picked up on its own, with no pairing step.

| Button | Does |
|---|---|
| D-pad / left stick | Move focus |
| A | Press what's focused, or start the game on a game tile |
| B | Close, or step back |
| X | Open a game's panel |
| Y | Favourite |
| LB / RB | Previous / next system |
| Start | Play the selection |

Movement is geometric rather than a hand-written map of what sits next to what.
Everything ordinarily interactive is a candidate, and the nearest thing in the
direction you pushed wins, weighted so travel along the axis beats drift across
it. A new button or a whole new skin is navigable the moment it renders, with no
second structure to keep in sync. Settings gets controller navigation for free.

The focus ring thickens while a pad is connected, since it has to be visible
from a sofa.

### Stats

Playtime is recorded per session. The library view shows total time, session
count, longest session, and jump-back-in and most-played lists that open the
game.

### Updates

Playdex checks for a new version a few seconds after launch, once the library
has drawn, so a slow or unreachable endpoint never delays startup. If it finds
one it says so in a corner card with the release notes. Nothing downloads until
you accept.

A failed check is silent, since being offline is normal for a desktop app. The
**Check for updates** button under **Settings → Appearance → About** reports the
real reason when you ask it directly.

Every update is verified against a public key compiled into the app before any
of it runs, so a tampered or mis-hosted file is refused rather than installed.

## Setup

```bash
npm install
npm run tauri dev      # development
npm run tauri build    # installer / bundled app
```

Needs Rust, and on Windows the MSVC build tools plus the WebView2 runtime.

### Credentials

Both providers are optional. Artwork works with no credentials at all through
libretro-thumbnails; these two add the text metadata it can't supply. Enter them
under **Settings → Metadata**. They're stored in the local SQLite database.

**ScreenScraper** (`screenscraper.fr`) needs two logins, which people often
confuse:

- A *developer* ID and password, requested from the site once you have an
  account. Nothing is bundled with this app, and without a dev key the API
  answers only a trickle of requests.
- Your *personal* ScreenScraper login, which raises the per-minute and daily
  quota. Contributors get a higher allowance.

**TheGamesDB** (`thegamesdb.net`) is a free API key with a monthly allowance,
used as the fallback.

**Test credentials** saves what you've entered and asks both providers whether
the keys work, reporting your remaining allowance, so a bad key shows up
immediately instead of as a wall of failures mid-scrape.

Scraping paces itself at roughly three requests a second and stops cleanly when
a provider says its quota is gone, telling you how far it got.

### Emulators

**Settings → Emulators** tries to find RetroArch for you. It checks the standard
install folders, Steam (via the registry, then following `libraryfolders.vdf`
across every drive, since Steam is often not on C:), every drive root, Scoop,
Chocolatey, the macOS app bundle, and Linux package and Flatpak paths, then
falls back to `PATH`.

Detection runs automatically the first time you open the tab with nothing set,
and **Detect RetroArch** re-runs it. It reports where it found the install and
how many cores are there, so a wrong hit is obvious. The cores folder is
inferred from the executable if you leave it blank. Paths are stripped of the
quotes Explorer's "Copy as path" adds, both on save and on use.

Per system you can override the core or switch to a standalone emulator:

```
"C:\Emulators\Dolphin\Dolphin.exe" -b -e "{rom}"
```

Systems with no libretro core in existence, the Switch for one, carry an empty
core list. Launching one tells you to point it at a standalone emulator instead
of blaming your RetroArch setup.

## Releasing

Updates are published as GitHub releases and found through the endpoint in
`plugins.updater.endpoints`. Pushing a version tag is the whole thing:

```bash
# bump `version` in package.json, src-tauri/tauri.conf.json
# and src-tauri/Cargo.toml so all three agree
git commit -am "Console skin, controller navigation, in-app updates"
git tag v0.2.0
git push && git push --tags
```

`.github/workflows/release.yml` builds the installer, signs it, and uploads it
with a `latest.json` manifest. **The commit message becomes the release notes**,
which is what the in-app notice shows, so write it for whoever reads it.

The repository has to be public. The updater fetches
`releases/latest/download/latest.json` with no credentials; on a private repo
that's a 404 and every check fails silently.

### The signing key

`tauri signer generate` produced a keypair. The public half is in
`tauri.conf.json`. The private half lives outside this repository at
`~/.playdex/updater.key`, in a folder that ignores itself so a stray
`git add -A` can't pick it up.

- **Anyone holding the private key can sign an update Playdex will trust and
  install.** Keep it in a password manager and in GitHub repository secrets,
  nowhere else.
- **Losing it can't be undone.** Installed copies only accept updates signed by
  the matching key, so a new keypair means replacing every existing install by
  hand. Back it up now, not later.

CI needs it as two secrets under *Settings → Secrets and variables → Actions*:
`TAURI_SIGNING_PRIVATE_KEY` (the file's contents) and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty unless you set one).

For a signed build locally:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.playdex/updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run tauri build
```

Without those, the build still produces an installer but stops before the `.sig`
files and says so. An unsigned release is one no existing copy can install.

## Layout

```
src-tauri/src/
  lib.rs          app setup, media:// protocol, command registration
  commands.rs     every command the UI can call
  db.rs           SQLite schema and queries
  scan.rs         folder walking, platform detection, title cleanup
  hashing.rs      one-pass CRC32/MD5/SHA1, reads into zip and 7z
  platforms.rs    system table: extensions, aliases, preferred cores
  romcheck.rs     telling ROMs from manuals, BIOS dumps and box art
  signature.rs    identifying a system from magic numbers in the header
  scrape/
    mod.rs        provider orchestration and fallback
    libretro.rs   artwork, no credentials needed
    screenscraper.rs
    thegamesdb.rs
  patch.rs        IPS/UPS/BPS patching with CRC verification
  hacks.rs        patch catalog import (folders and 7z bundles)
  detect.rs       finding an installed RetroArch
  cheats.rs       Game Genie codes and RetroArch's cheat format
  saves.rs        save files and save states
  media.rs        artwork download and cache
  launch.rs       command construction, archive extraction, playtime

src/
  App.tsx         state, event wiring, controller handling
  api.ts          typed wrappers over the Tauri commands
  gamepad.ts      pad polling and geometric focus movement
  update.ts       update check, download, install
  skins/          the three layouts, and what they're all handed
  components/     grid, detail panel, settings, toasts, modals
```

## Where hacks come from

ROMhacking.net went read-only in 2024 and released its database and file archive
to the Internet Archive, where it lives as the *ROM Hack Patch Archive*: 43
files, around 35 GB, packaged as per-system 7z bundles rather than a queryable
catalog. That's why importing is a one-time local operation instead of a live
API. The active successor community is romhack.ing.

Patches only ever carry the hack author's own changes. The base ROM comes from
you.

## Ideas

- **DAT matching.** Verify dumps against No-Intro/Redump DATs and flag bad
  dumps, renames and duplicates.
- **M3U grouping.** Collapse multi-disc games into one entry.
- **Save state and screenshot browsing** per game.

## Notes

- Removing a game removes the library entry only. The file on disk is never
  touched. The single exception is **Unpack and keep only the ROM**, which
  deletes an archive after replacing it with the ROM from inside, and says so
  before it does.
- The library lives in the app data directory next to the artwork cache, so
  deleting that folder resets everything without touching your ROMs.
