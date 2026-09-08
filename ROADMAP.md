# Roadmap

Things worth building, roughly in the order they'd help. Checked items
have shipped; the version they shipped in is in brackets.

## Now

- [ ] **Playlists and collections.** Hand-picked lists that cut across
      systems - "beat these", "co-op", "for the kids".
- [ ] **Show the artwork we already have.** The scraper fetches
      title-screen and in-game shots and nothing displays them.
- [ ] **Frontend tests.** 5,900 lines of TypeScript, no tests. The Rust
      side has 93.

## Next

- [ ] **Per-game emulator arguments.** The override takes a whole command;
      sometimes all you want is one extra flag on the system's.
- [ ] **Read a `.cue` sheet's own track list** rather than only the files
      it names, so a multi-track disc reports its real size.

## Later

- [ ] **RetroAchievements.** Log in, show what a game has, show what you
      have got.
- [ ] **Saved filters.** "Unplayed SNES games under 2 MB", kept around.
- [ ] **Raise the 512 MB hash ceiling.** It is what stops DAT verification
      from working on disc images, which is most of what it would be for.
- [ ] **Run it against a real library.** Thousands of files, not the
      handful used while building this.

## Done

- [x] Multi-disc games gathered into one entry, with an `.m3u` the emulator
      swaps discs from [0.8.0]
- [x] Raw `.bin` tracks read as the discs they are, with no `.cue` needed
      [0.8.0]
- [x] Per-game emulator override, which cheats and saves follow [0.8.0]
- [x] Adding a folder in Settings scans it, the way dropping one already
      did [0.7.2]
- [x] Smooth scrolling on the console skin [0.7.1]
- [x] Sony disc identification - PS1, PS2, PSP read out of ISO 9660 [0.7.0]
- [x] DAT verification against No-Intro and Redump catalogues [0.6.0]
- [x] Folder-based ROM collections, cue sheets and their tracks [0.5.x]
- [x] Drag and drop a ROM or a folder onto the window [0.4.0]
- [x] Extraction cache with a size limit, and deleting the original
      archive after unpacking [0.4.x]
- [x] Three appearances - desktop, console, and library - plus controller
      navigation and remapping [0.3.0]
- [x] Metadata fetched automatically after a scan [0.2.0]
- [x] In-app updates [0.2.0]
