/**
 * A showcase layout: a row of tiles pinned near the top, and whatever is
 * selected spread out underneath it.
 *
 * The shape of a current-generation console home screen, drawn here rather
 * than lifted. It differs from the handheld skin in where the weight sits.
 * There, the row is the centre of the screen and everything else is trim.
 * Here the row is a strip along the top, the selected tile grows away from
 * its neighbours, and the space below belongs entirely to that one game.
 */

import { useEffect, useMemo, useRef } from "react";

import { artUrl, formatPlaytime } from "../api";
import SystemIcon from "../components/SystemIcon";
import type { Game } from "../types";
import type { ShellProps } from "./shell";
import { bringIntoView } from "../gamepad";

export default function ShowcaseShell(props: ShellProps) {
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
  } = props;

  const rowRef = useRef<HTMLDivElement | null>(null);

  const systems = useMemo(
    () => platforms.filter((p) => p.gameCount > 0),
    [platforms],
  );

  useEffect(() => {
    if (selectedId == null || !rowRef.current) return;
    const tile = rowRef.current.querySelector<HTMLElement>(
      `[data-game-id="${selectedId}"]`,
    );
    if (tile) bringIntoView(tile);
  }, [selectedId]);

  const hero =
    artUrl(selected?.screenshotPath ?? null) ?? artUrl(selected?.coverPath ?? null);

  return (
    <div className="sc-shell">
      {hero && (
        <div className="sc-hero" aria-hidden="true">
          <img src={hero} alt="" />
          <div className="sc-hero-fade" />
        </div>
      )}

      <header className="sc-top">
        <div className="sc-filters">
          <button
            className={`sc-filter ${platform === null && !favoritesOnly ? "on" : ""}`}
            onClick={() => onSelectPlatform(null)}
            data-nav
          >
            All
          </button>
          <button
            className={`sc-filter ${favoritesOnly ? "on" : ""}`}
            onClick={onToggleFavorites}
            data-nav
          >
            Favourites
          </button>
          {systems.map((p) => (
            <button
              key={p.slug}
              className={`sc-filter ${platform === p.slug ? "on" : ""}`}
              onClick={() => onSelectPlatform(p.slug)}
              data-nav
            >
              {p.name}
            </button>
          ))}
        </div>

        <div className="sc-top-right">
          <input
            className="sc-search"
            value={search}
            placeholder="Search"
            onChange={(e) => onSearch(e.target.value)}
            data-nav
          />
          <button className="sc-icon-btn" onClick={onScan} data-nav title="Scan">
            Scan
          </button>
          <button className="sc-icon-btn" onClick={onOpenStats} data-nav title="Play history">
            Stats
          </button>
          <button
            className="sc-icon-btn"
            onClick={onOpenSettings}
            data-nav
            title="Settings"
          >
            Settings
          </button>
        </div>
      </header>

      {error && <div className="error-banner sc-error">{error}</div>}

      {isEmptyLibrary ? (
        <div className="sc-empty">
          <h1>Your library is empty</h1>
          <p>Add the folders where your ROMs live, then run a scan.</p>
          <button className="sc-play" onClick={onOpenSettings} data-nav data-nav-default>
            Add a ROM folder
          </button>
        </div>
      ) : (
        <>
          <div className="sc-row" ref={rowRef}>
            {games.length === 0 ? (
              <div className="sc-nothing">Nothing matches that search.</div>
            ) : (
              games.map((game) => (
                <Tile
                  key={game.id}
                  game={game}
                  on={game.id === selectedId}
                  onSelect={onSelect}
                  onLaunch={onLaunch}
                />
              ))
            )}
          </div>

          <div className="sc-detail">
            {selected ? (
              <>
                <h1 className="sc-title">{selected.title}</h1>
                <p className="sc-meta">
                  {[
                    selected.developer,
                    selected.releaseDate?.slice(0, 4),
                    selected.genre,
                    selected.players,
                  ]
                    .filter(Boolean)
                    .join("   ·   ")}
                </p>
                <p className="sc-played">{formatPlaytime(selected.playSeconds)}</p>
                {selected.description && (
                  <p className="sc-desc">{selected.description}</p>
                )}
                <div className="sc-actions">
                  <button
                    className="sc-play"
                    onClick={() => onLaunch(selected.id)}
                    data-nav
                  >
                    Play
                  </button>
                  <button
                    className="sc-secondary"
                    onClick={() => onOpenDetail(selected.id)}
                    data-nav
                  >
                    Manage
                  </button>
                </div>
              </>
            ) : (
              <p className="sc-meta">{games.length} games</p>
            )}
          </div>
        </>
      )}

      {padConnected && (
        <div className="sc-hints">
          <span>
            <b>A</b> Play
          </span>
          <span>
            <b>X</b> Manage
          </span>
          <span>
            <b>Y</b> Favourite
          </span>
          <span>
            <b>LB/RB</b> System
          </span>
        </div>
      )}
    </div>
  );
}

function Tile({
  game,
  on,
  onSelect,
  onLaunch,
}: {
  game: Game;
  on: boolean;
  onSelect: (id: number) => void;
  onLaunch: (id: number) => void;
}) {
  const cover = artUrl(game.coverPath);
  return (
    <button
      className={`sc-tile ${on ? "on" : ""}`}
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
        <span className="sc-tile-fallback">
          <SystemIcon platform={game.platform} size={24} />
          <span>{game.title}</span>
        </span>
      )}
      {game.favorite && <span className="sc-star">★</span>}
    </button>
  );
}
