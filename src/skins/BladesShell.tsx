/**
 * Blades: panels standing side by side, one open and the rest turned edge-on.
 *
 * The shape of a mid-2000s console dashboard, rebuilt here rather than
 * copied. What carries it is that the panels never go away — the ones you are
 * not in stay on screen as coloured spines, so moving between them reads as
 * sliding a stack apart rather than navigating to a different page.
 *
 * Back closes whichever blade is open and returns to the first, which is why
 * this skin claims the Back button: that is a view inside the skin and the
 * app knows nothing about it.
 */

import { useEffect, useMemo, useRef, useState } from "react";

import { artUrl, formatPlaytime } from "../api";
import SystemIcon from "../components/SystemIcon";
import type { Game } from "../types";
import type { ShellProps } from "./shell";
import { bringIntoView } from "../gamepad";

const BLADES = [
  { key: "games", label: "Games" },
  { key: "systems", label: "Systems" },
  { key: "game", label: "Selected" },
  { key: "more", label: "Romcade" },
] as const;

type BladeKey = (typeof BLADES)[number]["key"];

export default function BladesShell(props: ShellProps) {
  const {
    games,
    platforms,
    platform,
    search,
    selected,
    selectedId,
    favoritesOnly,
    error,
    isEmptyLibrary,
    padConnected,
    onSelectPlatform,
    onToggleFavorites,
    onSearch,
    onSelect,
    onLaunch,
    onOpenDetail,
    onOpenSettings,
    onOpenStats,
    onScan,
    registerBack,
  } = props;

  const [open, setOpen] = useState<BladeKey>("games");
  const gridRef = useRef<HTMLDivElement | null>(null);

  const systems = useMemo(
    () => platforms.filter((p) => p.gameCount > 0),
    [platforms],
  );

  // Back steps out of an opened blade before the app closes anything.
  useEffect(() => {
    registerBack(() => {
      if (open !== "games") {
        setOpen("games");
        return true;
      }
      return false;
    });
    return () => registerBack(null);
  }, [registerBack, open]);

  useEffect(() => {
    if (selectedId == null || open !== "games" || !gridRef.current) return;
    const tile = gridRef.current.querySelector<HTMLElement>(
      `[data-game-id="${selectedId}"]`,
    );
    if (tile) bringIntoView(tile);
  }, [selectedId, open]);

  return (
    <div className="bl-shell">
      <div className="bl-sky" aria-hidden="true" />

      <header className="bl-top">
        <span className="bl-brand">Romcade</span>
        <input
          className="bl-search"
          value={search}
          placeholder="Search"
          onChange={(e) => onSearch(e.target.value)}
          data-nav
        />
        <span className="bl-count">{games.length} games</span>
      </header>

      {error && <div className="error-banner bl-error">{error}</div>}

      <div className="bl-blades">
        {BLADES.map((blade) => {
          const active = blade.key === open;
          return (
            <section
              key={blade.key}
              className={`bl-blade bl-${blade.key} ${active ? "open" : "shut"}`}
            >
              {active ? (
                <>
                  <h2 className="bl-head">{blade.label}</h2>
                  <div className="bl-content">
                    {blade.key === "games" && (
                      <GamesBlade
                        games={games}
                        selectedId={selectedId}
                        isEmptyLibrary={isEmptyLibrary}
                        gridRef={gridRef}
                        onSelect={onSelect}
                        onLaunch={onLaunch}
                        onOpenSettings={onOpenSettings}
                      />
                    )}

                    {blade.key === "systems" && (
                      <div className="bl-list">
                        <button
                          className={`bl-row ${platform === null && !favoritesOnly ? "on" : ""}`}
                          onClick={() => onSelectPlatform(null)}
                          data-nav
                          data-nav-default
                        >
                          All games
                        </button>
                        <button
                          className={`bl-row ${favoritesOnly ? "on" : ""}`}
                          onClick={onToggleFavorites}
                          data-nav
                        >
                          Favourites
                        </button>
                        {systems.map((p) => (
                          <button
                            key={p.slug}
                            className={`bl-row ${platform === p.slug ? "on" : ""}`}
                            onClick={() => onSelectPlatform(p.slug)}
                            data-nav
                          >
                            <SystemIcon platform={p.slug} size={16} />
                            <span>{p.name}</span>
                            <span className="bl-row-count">{p.gameCount}</span>
                          </button>
                        ))}
                      </div>
                    )}

                    {blade.key === "game" && (
                      <SelectedBlade
                        game={selected}
                        onLaunch={onLaunch}
                        onOpenDetail={onOpenDetail}
                      />
                    )}

                    {blade.key === "more" && (
                      <div className="bl-list">
                        <button className="bl-row" onClick={onScan} data-nav data-nav-default>
                          Scan for games
                        </button>
                        <button className="bl-row" onClick={onOpenStats} data-nav>
                          Play history
                        </button>
                        <button className="bl-row" onClick={onOpenSettings} data-nav>
                          Settings
                        </button>
                      </div>
                    )}
                  </div>
                </>
              ) : (
                <button
                  className="bl-spine"
                  onClick={() => setOpen(blade.key)}
                  data-nav
                  title={blade.label}
                >
                  <span className="bl-spine-label">{blade.label}</span>
                </button>
              )}
            </section>
          );
        })}
      </div>

      {padConnected && (
        <div className="bl-hints">
          <span>
            <b>A</b> Open
          </span>
          <span>
            <b>B</b> Back a blade
          </span>
          <span>
            <b>Y</b> Favourite
          </span>
        </div>
      )}
    </div>
  );
}

function GamesBlade({
  games,
  selectedId,
  isEmptyLibrary,
  gridRef,
  onSelect,
  onLaunch,
  onOpenSettings,
}: {
  games: Game[];
  selectedId: number | null;
  isEmptyLibrary: boolean;
  gridRef: React.RefObject<HTMLDivElement | null>;
  onSelect: (id: number) => void;
  onLaunch: (id: number) => void;
  onOpenSettings: () => void;
}) {
  if (isEmptyLibrary) {
    return (
      <div className="bl-empty">
        <p>Your library is empty. Add the folders where your ROMs live.</p>
        <button className="bl-row" onClick={onOpenSettings} data-nav data-nav-default>
          Add a ROM folder
        </button>
      </div>
    );
  }
  if (games.length === 0) {
    return <div className="bl-empty">Nothing matches that search.</div>;
  }
  return (
    <div className="bl-grid" ref={gridRef}>
      {games.map((game) => {
        const on = game.id === selectedId;
        const cover = artUrl(game.coverPath);
        return (
          <button
            key={game.id}
            className={`bl-tile ${on ? "on" : ""}`}
            data-game-id={game.id}
            data-nav
            {...(on ? { "data-nav-default": "" } : {})}
            onFocus={() => onSelect(game.id)}
            onClick={() => onSelect(game.id)}
            onDoubleClick={() => onLaunch(game.id)}
            title={game.title}
          >
            {cover ? (
              <img src={cover} alt="" loading="lazy" />
            ) : (
              <span className="bl-tile-fallback">
                <SystemIcon platform={game.platform} size={22} />
                <span>{game.title}</span>
              </span>
            )}
            {game.favorite && <span className="bl-star">★</span>}
          </button>
        );
      })}
    </div>
  );
}

function SelectedBlade({
  game,
  onLaunch,
  onOpenDetail,
}: {
  game: Game | null;
  onLaunch: (id: number) => void;
  onOpenDetail: (id: number) => void;
}) {
  if (!game) return <div className="bl-empty">Nothing selected.</div>;
  const art = artUrl(game.coverPath);
  return (
    <div className="bl-selected">
      <div className="bl-selected-art">
        {art ? <img src={art} alt="" /> : <SystemIcon platform={game.platform} size={48} />}
      </div>
      <h3>{game.title}</h3>
      <p className="bl-selected-meta">
        {[game.developer, game.releaseDate?.slice(0, 4), game.genre]
          .filter(Boolean)
          .join("  ·  ")}
      </p>
      <p className="bl-selected-meta">{formatPlaytime(game.playSeconds)}</p>
      {game.description && <p className="bl-selected-desc">{game.description}</p>}
      <div className="bl-selected-actions">
        <button className="bl-play" onClick={() => onLaunch(game.id)} data-nav data-nav-default>
          Play
        </button>
        <button className="bl-row" onClick={() => onOpenDetail(game.id)} data-nav>
          Manage
        </button>
      </div>
    </div>
  );
}
