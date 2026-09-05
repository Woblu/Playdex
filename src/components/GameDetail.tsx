import { Fragment, useEffect, useState } from "react";

import * as api from "../api";
import { artUrl, formatDate, formatPlaytime, formatSize } from "../api";
import type {
  Cheat,
  SaveEntry,
  Game,
  HackPreview,
  PatchEntry,
  PlatformInfo,
  RetroArchCheats,
} from "../types";
import SystemIcon from "./SystemIcon";

interface Props {
  game: Game;
  busy: boolean;
  onClose: () => void;
  onLaunch: () => void;
  onToggleFavorite: () => void;
  onScrape: () => Promise<string | undefined>;
  onRemove: () => void;
  onSetPlatform: (slug: string) => void;
  onHackAdded: () => void;
}

export default function GameDetail({
  game,
  busy,
  onClose,
  onLaunch,
  onToggleFavorite,
  onScrape,
  onRemove,
  onSetPlatform,
  onHackAdded,
}: Props) {
  const [allPlatforms, setAllPlatforms] = useState<PlatformInfo[]>([]);
  const [command, setCommand] = useState<string | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [scrapeNote, setScrapeNote] = useState<string | null>(null);

  const [patchPath, setPatchPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<HackPreview | null>(null);
  const [hackTitle, setHackTitle] = useState("");
  const [hackError, setHackError] = useState<string | null>(null);
  const [hackBusy, setHackBusy] = useState(false);
  const [catalogHacks, setCatalogHacks] = useState<PatchEntry[]>([]);
  const [cheats, setCheats] = useState<Cheat[]>([]);
  const [cheatBusy, setCheatBusy] = useState(false);
  const [cheatNote, setCheatNote] = useState<string | null>(null);
  const [cheatError, setCheatError] = useState<string | null>(null);
  const [raCheats, setRaCheats] = useState<RetroArchCheats | null>(null);
  const [cheatQuery, setCheatQuery] = useState("");
  const [saves, setSaves] = useState<SaveEntry[]>([]);
  const [saveBusy, setSaveBusy] = useState(false);
  const [saveNote, setSaveNote] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  // Some games carry several hundred cheats — Super Mario Bros has 711 — so
  // the ones in use are pinned above and the rest are searched, not scrolled.
  const activeCheats = cheats.filter((c) => c.enabled);
  const needle = cheatQuery.trim().toLowerCase();
  const matchingCheats = cheats.filter(
    (c) =>
      !c.enabled &&
      (needle === "" ||
        c.description.toLowerCase().includes(needle) ||
        c.code.toLowerCase().includes(needle)),
  );
  const visibleCheats = matchingCheats.slice(0, 60);

  const clearHack = () => {
    setPatchPath(null);
    setPreview(null);
    setHackTitle("");
    setHackError(null);
  };

  const choosePatch = async () => {
    setHackError(null);
    try {
      const path = await api.pickFile();
      if (!path) return;
      const info = await api.inspectPatch(game.id, path);
      setPatchPath(path);
      setPreview(info);
      setHackTitle(info.suggestedTitle);
    } catch (e) {
      setHackError(api.errorMessage(e));
    }
  };

  const applyCatalogHack = async (entry: PatchEntry) => {
    setHackBusy(true);
    setHackError(null);
    try {
      await api.applyCatalogPatch(game.id, entry.id, entry.name);
      onHackAdded();
    } catch (e) {
      setHackError(api.errorMessage(e));
    } finally {
      setHackBusy(false);
    }
  };

  const searchCheats = async () => {
    setCheatBusy(true);
    setCheatError(null);
    setCheatNote(null);
    try {
      const found = await api.findCheats(game.id);
      setCheats(found);
      if (found.length === 0) {
        setCheatNote("No cheats published for this ROM.");
      }
    } catch (e) {
      setCheatError(api.errorMessage(e));
    } finally {
      setCheatBusy(false);
    }
  };

  const refreshSaves = () =>
    api.listSaves(game.id).then(setSaves).catch(() => setSaves([]));

  const backUpSaves = async () => {
    setSaveBusy(true);
    setSaveError(null);
    try {
      setSaveNote(await api.backUpSaves(game.id));
    } catch (e) {
      setSaveError(api.errorMessage(e));
    } finally {
      setSaveBusy(false);
    }
  };

  const removeState = async (entry: SaveEntry) => {
    setSaveBusy(true);
    setSaveError(null);
    try {
      await api.deleteSaveState(entry.path);
      setSaveNote(`Deleted ${entry.name}`);
      await refreshSaves();
    } catch (e) {
      setSaveError(api.errorMessage(e));
    } finally {
      setSaveBusy(false);
    }
  };

  const toggleAll = async (enabled: boolean) => {
    setCheatBusy(true);
    setCheatError(null);
    try {
      setCheats(await api.setAllCheats(game.id, enabled));
      setCheatNote(null);
    } catch (e) {
      setCheatError(api.errorMessage(e));
    } finally {
      setCheatBusy(false);
    }
  };

  const toggleCheat = async (cheat: Cheat) => {
    const next = !cheat.enabled;
    setCheats((prev) =>
      prev.map((c) => (c.index === cheat.index ? { ...c, enabled: next } : c)),
    );
    try {
      await api.setCheat(game.id, cheat.index, next);
    } catch (e) {
      setCheatError(api.errorMessage(e));
    }
  };

  const writeCheats = async () => {
    setCheatBusy(true);
    setCheatError(null);
    try {
      const path = await api.saveCheats(game.id);
      setCheatNote(`Saved to ${path}`);
    } catch (e) {
      setCheatError(api.errorMessage(e));
    } finally {
      setCheatBusy(false);
    }
  };

  const applyPatch = async () => {
    if (!patchPath) return;
    setHackBusy(true);
    setHackError(null);
    try {
      await api.addHack(game.id, patchPath, hackTitle);
      clearHack();
      onHackAdded();
    } catch (e) {
      setHackError(api.errorMessage(e));
    } finally {
      setHackBusy(false);
    }
  };

  useEffect(() => {
    void api.knownPlatforms().then(setAllPlatforms);
  }, []);

  // Show what will actually run, so a bad emulator path is visible up front.
  useEffect(() => {
    setCommand(null);
    setCommandError(null);
    setScrapeNote(null);
    setPatchPath(null);
    setPreview(null);
    setHackTitle("");
    setHackError(null);
    api
      .previewLaunch(game.id)
      .then(setCommand)
      .catch((e) => setCommandError(api.errorMessage(e)));
    api
      .patchesForGame(game.id)
      .then(setCatalogHacks)
      .catch(() => setCatalogHacks([]));
    setCheatNote(null);
    setCheatError(null);
    setCheatQuery("");
    api.listCheats(game.id).then(setCheats).catch(() => setCheats([]));
    api.retroarchCheatStatus().then(setRaCheats).catch(() => setRaCheats(null));
    setSaveNote(null);
    setSaveError(null);
    api.listSaves(game.id).then(setSaves).catch(() => setSaves([]));
  }, [game.id, game.platform]);

  const hero = artUrl(game.screenshotPath) ?? artUrl(game.coverPath);

  const rows: Array<[string, string | null]> = [
    ["System", api.platformName(allPlatforms, game.platform)],
    ["Developer", game.developer],
    ["Publisher", game.publisher],
    ["Released", game.releaseDate],
    ["Genre", game.genre],
    ["Players", game.players],
    ["Region", game.region],
    ["Rating", game.rating ? `${game.rating.toFixed(1)} / 5` : null],
    ["Playtime", formatPlaytime(game.playSeconds)],
    ["Last played", game.lastPlayed ? formatDate(game.lastPlayed) : "Never"],
    ["Size", formatSize(game.size)],
    ["Metadata", game.scrapeSource ?? "not fetched"],
  ];

  return (
    <aside className="detail">
      <div className="detail-hero">
        <button className="detail-close" onClick={onClose} aria-label="Close">
          ✕
        </button>
        {hero && <img src={hero} alt="" />}
      </div>

      <div className="detail-body">
        <h1 className="detail-title">{game.title}</h1>

        <div className="detail-actions">
          <button className="btn primary" onClick={onLaunch} disabled={busy}>
            ▶ Play
          </button>
          <button className="btn" onClick={onToggleFavorite} disabled={busy}>
            {game.favorite ? "★ Favorited" : "☆ Favorite"}
          </button>
          <button
            className="btn"
            disabled={busy}
            onClick={async () => {
              const note = await onScrape();
              if (note) setScrapeNote(note);
            }}
          >
            Refetch
          </button>
        </div>

        {scrapeNote && <div className="notice">{scrapeNote}</div>}
        {commandError && <div className="error-banner">{commandError}</div>}

        <div className="meta-grid">
          {rows
            .filter(([, value]) => value)
            .map(([key, value]) => (
              <Fragment key={key}>
                <div className="meta-key">{key}</div>
                <div className="meta-val">{value}</div>
              </Fragment>
            ))}
        </div>

        {game.description && (
          <>
            <div className="section-title">About</div>
            <div className="description">{game.description}</div>
          </>
        )}

        <div className="section-title">System</div>
        <div className="row">
          <SystemIcon platform={game.platform} size={20} className="system-mark" />
          <select
            value={game.platform}
            onChange={(e) => onSetPlatform(e.target.value)}
          >
          {!allPlatforms.some((p) => p.slug === game.platform) && (
            <option value={game.platform}>{game.platform}</option>
          )}
          {allPlatforms.map((p) => (
            <option key={p.slug} value={p.slug}>
              {p.name}
            </option>
          ))}
          </select>
        </div>
        <div className="hint" style={{ marginTop: 6 }}>
          Changing the system re-queues this game for metadata and picks a
          different core.
        </div>

        <div className="section-title">File</div>
        <div className="path-box">{game.path}</div>

        {command && (
          <>
            <div className="section-title">Launch command</div>
            <div className="path-box">{command}</div>
          </>
        )}

        {game.baseGameId ? (
          <>
            <div className="section-title">ROM hack</div>
            <div className="hint">
              Built from another ROM in your library. The original was not
              modified.
            </div>
            {game.patchPath && (
              <div className="path-box" style={{ marginTop: 6 }}>
                {game.patchPath}
              </div>
            )}
          </>
        ) : (
          <>
            <div className="section-title">ROM hacks</div>

            {catalogHacks.length > 0 && !preview && (
              <>
                <div className="hint" style={{ marginBottom: 8 }}>
                  {catalogHacks.length} patch
                  {catalogHacks.length === 1 ? "" : "es"} in your catalog match
                  this ROM's checksum exactly.
                </div>
                {catalogHacks.map((entry) => (
                  <div className="folder-row" key={entry.id}>
                    <div className="folder-path">
                      <div style={{ color: "var(--text)" }}>{entry.name}</div>
                      <div className="card-sub">
                        {entry.format}
                        {entry.targetHint ? ` · ${entry.targetHint}` : ""}
                      </div>
                    </div>
                    <button
                      className="btn small"
                      disabled={hackBusy}
                      onClick={() => applyCatalogHack(entry)}
                    >
                      Apply
                    </button>
                  </div>
                ))}
              </>
            )}

            {!preview && (
              <>
                <button className="btn small" onClick={choosePatch}>
                  Add ROM hack…
                </button>
                <div className="hint" style={{ marginTop: 6 }}>
                  Pick an IPS, UPS or BPS patch. It is applied to a copy — this
                  ROM is never modified — and the result joins your library as
                  its own game.
                </div>
              </>
            )}

            {preview && (
              <div
                className="notice"
                style={{
                  borderLeftColor: preview.compatible
                    ? "var(--success)"
                    : "var(--danger)",
                }}
              >
                <strong>{preview.format} patch</strong>
                <div style={{ marginTop: 4 }}>{preview.message}</div>

                <div className="field" style={{ margin: "12px 0 8px" }}>
                  <label>Name in your library</label>
                  <input
                    value={hackTitle}
                    onChange={(e) => setHackTitle(e.target.value)}
                  />
                </div>

                <div className="row">
                  <button
                    className="btn small primary"
                    onClick={applyPatch}
                    disabled={hackBusy || !preview.compatible}
                  >
                    {hackBusy ? "Applying…" : "Apply patch"}
                  </button>
                  <button className="btn small" onClick={clearHack}>
                    Cancel
                  </button>
                </div>
              </div>
            )}

            {hackError && <div className="error-banner">{hackError}</div>}
          </>
        )}

        <div className="section-title">Cheats</div>

        {cheats.length === 0 ? (
          <>
            <button className="btn small" onClick={searchCheats} disabled={cheatBusy}>
              {cheatBusy ? "Looking…" : "Find cheats"}
            </button>
            <div className="hint" style={{ marginTop: 6 }}>
              Searches libretro's cheat database for Game Genie codes matching
              this ROM. Free, no account needed.
            </div>
          </>
        ) : (
          <>
            <input
              value={cheatQuery}
              placeholder={"Search " + cheats.length + " cheats…"}
              onChange={(e) => setCheatQuery(e.target.value)}
            />

            <div className="cheat-summary">
              <span>
                <strong
                  style={{
                    color: activeCheats.length ? "var(--accent)" : undefined,
                  }}
                >
                  {activeCheats.length}
                </strong>{" "}
                on of {cheats.length}
              </span>
              <span className="spacer" />
              {activeCheats.length > 0 && (
                <button
                  className="btn small danger"
                  onClick={() => void toggleAll(false)}
                  disabled={cheatBusy}
                >
                  Turn all off
                </button>
              )}
            </div>

            {activeCheats.length > 0 && (
              <>
                <div className="cheat-group">On</div>
                <div className="cheat-list">
                  {activeCheats.map((cheat) => (
                    <label className="cheat" key={cheat.index}>
                      <input
                        type="checkbox"
                        checked
                        onChange={() => void toggleCheat(cheat)}
                      />
                      <span>
                        <span className="cheat-desc">{cheat.description}</span>
                        <span className="cheat-code">{cheat.code}</span>
                      </span>
                    </label>
                  ))}
                </div>
              </>
            )}

            <div className="cheat-group">
              {needle ? "Matches (" + matchingCheats.length + ")" : "Available"}
            </div>

            {matchingCheats.length === 0 ? (
              <div className="hint">
                {needle
                  ? "Nothing matches that search."
                  : "Everything is switched on."}
              </div>
            ) : (
              <>
                <div className="cheat-list">
                  {visibleCheats.map((cheat) => (
                    <label className="cheat" key={cheat.index}>
                      <input
                        type="checkbox"
                        checked={false}
                        onChange={() => void toggleCheat(cheat)}
                      />
                      <span>
                        <span className="cheat-desc">{cheat.description}</span>
                        <span className="cheat-code">{cheat.code}</span>
                      </span>
                    </label>
                  ))}
                </div>
                {matchingCheats.length > visibleCheats.length && (
                  <div className="hint" style={{ marginTop: 6 }}>
                    Showing {visibleCheats.length} of {matchingCheats.length}.
                    Search to narrow it down.
                  </div>
                )}
              </>
            )}

            <div className="row" style={{ marginTop: 12 }}>
              <button className="btn small" onClick={searchCheats} disabled={cheatBusy}>
                Refresh
              </button>
              <button className="btn small" onClick={writeCheats} disabled={cheatBusy}>
                Write now
              </button>
            </div>

            <div className="hint" style={{ marginTop: 6 }}>
              Whatever is switched on here is written to RetroArch when you
              press Play, under every name RetroArch might use for this ROM, so
              it cannot load a stale file instead. "Write now" is only needed if
              you are launching from RetroArch directly rather than from here.
              {raCheats && !raCheats.autoApply && activeCheats.length > 0 && (
                <>
                  {" "}
                  RetroArch's "Auto-Apply Cheats During Game Load" is currently
                  off; playing from here turns it on, since a cheat file it
                  never reads would do nothing.
                </>
              )}
            </div>
          </>
        )}

        {cheatNote && (
          <div className="path-box" style={{ marginTop: 8 }}>
            {cheatNote}
          </div>
        )}
        {cheatError && <div className="error-banner">{cheatError}</div>}

        <div className="section-title">Saves</div>

        {saves.length === 0 ? (
          <div className="hint">
            No saves or save states found yet. They appear here once the game
            has written one.
          </div>
        ) : (
          <>
            <div className="cheat-summary">
              <span>
                {saves.filter((s) => s.kind === "save").length} save
                {saves.filter((s) => s.kind === "save").length === 1 ? "" : "s"}
                {", "}
                {saves.filter((s) => s.kind === "state").length} state
                {saves.filter((s) => s.kind === "state").length === 1 ? "" : "s"}
              </span>
              <span className="spacer" />
              <button
                className="btn small"
                onClick={backUpSaves}
                disabled={saveBusy}
              >
                Back up all
              </button>
            </div>

            <div className="save-list">
              {saves.map((entry) => (
                <div className="save-row" key={entry.path}>
                  {entry.screenshot ? (
                    <img
                      className="save-shot"
                      src={artUrl(entry.screenshot) ?? undefined}
                      alt=""
                    />
                  ) : (
                    <span
                      className={
                        "save-kind " + (entry.kind === "save" ? "battery" : "snap")
                      }
                    >
                      {entry.kind === "save" ? "SAV" : "ST"}
                      {entry.slot !== null ? entry.slot : ""}
                    </span>
                  )}

                  <span className="save-meta">
                    <span className="save-name">{entry.name}</span>
                    <span className="card-sub">
                      {formatSize(entry.size)} · {formatDate(entry.modified)}
                      {entry.kind === "save" ? " · game progress" : " · snapshot"}
                    </span>
                  </span>

                  {entry.kind === "state" && (
                    <button
                      className="btn small danger"
                      disabled={saveBusy}
                      onClick={() => void removeState(entry)}
                    >
                      Delete
                    </button>
                  )}
                </div>
              ))}
            </div>

            <div className="hint" style={{ marginTop: 6 }}>
              Back up copies everything into the app data folder, dated. Only
              save states can be deleted here — battery saves hold your actual
              progress.
            </div>
          </>
        )}

        {saveNote && (
          <div className="path-box" style={{ marginTop: 8 }}>
            {saveNote}
          </div>
        )}
        {saveError && <div className="error-banner">{saveError}</div>}

        <div className="section-title">Manage</div>
        <div className="row">
          <button
            className="btn small"
            onClick={() => void api.revealGame(game.id)}
          >
            Show in folder
          </button>
          <button className="btn small danger" onClick={onRemove}>
            Remove from library
          </button>
        </div>
        <div className="hint" style={{ marginTop: 6 }}>
          Removing only forgets the entry here. The file on disk is untouched.
        </div>
      </div>
    </aside>
  );
}
