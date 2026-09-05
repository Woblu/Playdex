import { artUrl } from "../api";
import type { Game } from "../types";
import SystemIcon from "./SystemIcon";

interface Props {
  games: Game[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onLaunch: (id: number) => void;
}

export default function GameGrid({
  games,
  selectedId,
  onSelect,
  onLaunch,
}: Props) {
  if (games.length === 0) {
    return (
      <div className="empty">
        <div className="empty-inner">
          <h2>Nothing matches</h2>
          <p>Try a different search, or clear the filters in the sidebar.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="grid">
      {games.map((game) => {
        const cover = artUrl(game.coverPath);
        return (
          <button
            key={game.id}
            className={`card ${selectedId === game.id ? "selected" : ""}`}
            data-game-id={game.id}
            {...(selectedId === game.id ? { "data-nav-default": "" } : {})}
            onClick={() => onSelect(game.id)}
            onDoubleClick={() => onLaunch(game.id)}
            title={`${game.title}\nDouble-click to play`}
          >
            <div className="card-art">
              {cover ? (
                <img src={cover} alt="" loading="lazy" />
              ) : (
                <div className="placeholder">
                  <SystemIcon platform={game.platform} size={30} />
                  <span>{game.title}</span>
                </div>
              )}
              {game.favorite && <span className="badge fav">★</span>}
              {game.scrapeStatus !== "ok" && (
                <span className="badge unscraped">no art</span>
              )}
            </div>
            <div>
              <div className="card-title">{game.title}</div>
              <div className="card-sub">
                {game.playSeconds > 0
                  ? `${Math.round(game.playSeconds / 60)} min played`
                  : game.region || " "}
              </div>
            </div>
          </button>
        );
      })}
    </div>
  );
}
