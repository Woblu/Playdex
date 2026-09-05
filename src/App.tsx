import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import * as api from "./api";
import { errorMessage } from "./api";
import type {
  Game,
  GameFilter,
  LibraryStats,
  PlatformInfo,
  ScanProgress,
  ScrapeProgress,
  SortKey,
} from "./types";

import Sidebar from "./components/Sidebar";
import TopBar from "./components/TopBar";
import GameGrid from "./components/GameGrid";
import GameDetail from "./components/GameDetail";
import SettingsModal from "./components/SettingsModal";
import HomebrewModal from "./components/HomebrewModal";
import ProgressToast from "./components/ProgressToast";

export default function App() {
  const [games, setGames] = useState<Game[]>([]);
  const [platforms, setPlatforms] = useState<PlatformInfo[]>([]);
  const [stats, setStats] = useState<LibraryStats | null>(null);

  const [platform, setPlatform] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [sort, setSort] = useState<SortKey>("title");
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [unscrapedOnly, setUnscrapedOnly] = useState(false);

  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [homebrewOpen, setHomebrewOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [scan, setScan] = useState<ScanProgress | null>(null);
  const [scrape, setScrape] = useState<ScrapeProgress | null>(null);
  const [nowPlaying, setNowPlaying] = useState<string | null>(null);

  const hideTimer = useRef<number | null>(null);

  const filter: GameFilter = useMemo(
    () => ({
      platform,
      search: debouncedSearch,
      sort,
      favoritesOnly,
      unscrapedOnly,
      showHidden: false,
    }),
    [platform, debouncedSearch, sort, favoritesOnly, unscrapedOnly],
  );

  useEffect(() => {
    const t = window.setTimeout(() => setDebouncedSearch(search), 180);
    return () => window.clearTimeout(t);
  }, [search]);

  const refreshGames = useCallback(async () => {
    try {
      setGames(await api.listGames(filter));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [filter]);

  const refreshSidebar = useCallback(async () => {
    try {
      const [p, s] = await Promise.all([api.listPlatforms(), api.libraryStats()]);
      setPlatforms(p);
      setStats(s);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    void refreshGames();
  }, [refreshGames]);

  useEffect(() => {
    void refreshSidebar();
  }, [refreshSidebar]);

  // Backend progress and launch events.
  useEffect(() => {
    const unlisteners: Array<Promise<() => void>> = [
      listen<ScanProgress>("scan-progress", (e) => {
        setScan(e.payload);
        if (e.payload.done) {
          void refreshGames();
          void refreshSidebar();
          scheduleHide(() => setScan(null));
        }
      }),
      listen<ScrapeProgress>("scrape-progress", (e) => {
        setScrape(e.payload);
        if (e.payload.done) {
          void refreshGames();
          void refreshSidebar();
          scheduleHide(() => setScrape(null));
        }
      }),
      listen<{ title: string }>("game-launched", (e) => {
        setNowPlaying(e.payload.title);
      }),
      listen<{ title: string }>("game-exited", () => {
        setNowPlaying(null);
        void refreshGames();
        void refreshSidebar();
      }),
    ];

    return () => {
      unlisteners.forEach((p) => void p.then((f) => f()));
    };
  }, [refreshGames, refreshSidebar]);

  function scheduleHide(fn: () => void) {
    if (hideTimer.current) window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(fn, 4000);
  }

  const selected = useMemo(
    () => games.find((g) => g.id === selectedId) ?? null,
    [games, selectedId],
  );

  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    setBusy(true);
    setError(null);
    try {
      return await fn();
    } catch (e) {
      setError(errorMessage(e));
      return undefined;
    } finally {
      setBusy(false);
    }
  }

  const handleScan = () =>
    run(async () => {
      await api.scanLibrary();
      await refreshGames();
      await refreshSidebar();
    });

  const handleScrape = () =>
    run(async () => {
      const summary = await api.scrapeLibrary(platform);
      setScrape((prev) =>
        prev ? { ...prev, done: true, status: summary } : prev,
      );
    });

  const handleLaunch = (id: number) => run(() => api.launchGame(id));

  const handleToggleFavorite = (game: Game) =>
    run(async () => {
      await api.setFavorite(game.id, !game.favorite);
      await refreshGames();
    });

  const handleScrapeOne = (id: number) =>
    run(async () => {
      const msg = await api.scrapeOne(id);
      await refreshGames();
      return msg;
    });

  const handleRemove = (id: number) =>
    run(async () => {
      await api.removeGame(id);
      setSelectedId(null);
      await refreshGames();
      await refreshSidebar();
    });

  const handleSetPlatform = (id: number, slug: string) =>
    run(async () => {
      await api.setGamePlatform(id, slug);
      await refreshGames();
      await refreshSidebar();
    });

  const isEmptyLibrary = stats?.totalGames === 0;

  return (
    <div className="app">
      <Sidebar
        platforms={platforms}
        stats={stats}
        selected={platform}
        favoritesOnly={favoritesOnly}
        unscrapedOnly={unscrapedOnly}
        onSelectPlatform={(slug) => {
          setPlatform(slug);
          setFavoritesOnly(false);
          setUnscrapedOnly(false);
        }}
        onToggleFavorites={() => {
          setFavoritesOnly((v) => !v);
          setUnscrapedOnly(false);
        }}
        onToggleUnscraped={() => {
          setUnscrapedOnly((v) => !v);
          setFavoritesOnly(false);
        }}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenHomebrew={() => setHomebrewOpen(true)}
      />

      <div className="main">
        <TopBar
          search={search}
          onSearch={setSearch}
          sort={sort}
          onSort={setSort}
          busy={busy}
          onScan={handleScan}
          onScrape={handleScrape}
          count={games.length}
        />

        <div className="content">
          {error && <div className="error-banner">{error}</div>}

          {isEmptyLibrary ? (
            <div className="empty">
              <div className="empty-inner">
                <h2>Your library is empty</h2>
                <p>
                  Add the folders where your ROMs live, then run a scan.
                  Playdex reads what is already on your disk — it identifies
                  each file by hash and fetches cover art and details for it.
                </p>
                <button
                  className="btn primary"
                  onClick={() => setSettingsOpen(true)}
                >
                  Add a ROM folder
                </button>
              </div>
            </div>
          ) : (
            <GameGrid
              games={games}
              selectedId={selectedId}
              onSelect={setSelectedId}
              onLaunch={handleLaunch}
            />
          )}
        </div>
      </div>

      {selected && (
        <GameDetail
          game={selected}
          onClose={() => setSelectedId(null)}
          onLaunch={() => handleLaunch(selected.id)}
          onToggleFavorite={() => handleToggleFavorite(selected)}
          onScrape={() => handleScrapeOne(selected.id)}
          onRemove={() => handleRemove(selected.id)}
          onSetPlatform={(slug) => handleSetPlatform(selected.id, slug)}
          onHackAdded={() => {
            void refreshGames();
            void refreshSidebar();
          }}
          busy={busy}
        />
      )}

      {settingsOpen && (
        <SettingsModal
          onClose={() => {
            setSettingsOpen(false);
            void refreshSidebar();
            void refreshGames();
          }}
        />
      )}

      {homebrewOpen && (
        <HomebrewModal
          onClose={() => setHomebrewOpen(false)}
          onInstalled={() => {
            void refreshGames();
            void refreshSidebar();
          }}
        />
      )}

      {(scan || scrape || nowPlaying) && (
        <ProgressToast
          scan={scan}
          scrape={scrape}
          nowPlaying={nowPlaying}
          onCancelScrape={() => void api.cancelScrape()}
        />
      )}
    </div>
  );
}
