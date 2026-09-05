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

import GameDetail from "./components/GameDetail";
import SettingsModal from "./components/SettingsModal";
import HomebrewModal from "./components/HomebrewModal";
import StatsModal from "./components/StatsModal";
import ProgressToast from "./components/ProgressToast";
import UpdateBanner from "./components/UpdateBanner";
import { checkForUpdate, type UpdateInfo } from "./update";

import LaunchboxShell from "./skins/LaunchboxShell";
import SwitchShell from "./skins/SwitchShell";
import SteamShell from "./skins/SteamShell";
import { DEFAULT_SKIN, isSkin, type ShellProps, type SkinName } from "./skins/shell";
import {
  activateFocused,
  focusFirst,
  focusInDirection,
  useGamepad,
} from "./gamepad";

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
  const [statsOpen, setStatsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [skin, setSkin] = useState<SkinName>(DEFAULT_SKIN);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updateDismissed, setUpdateDismissed] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);
  const [padConnected, setPadConnected] = useState(false);

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

  // The chosen skin lives in the same settings table as everything else, so
  // it survives a restart without a second store.
  const loadSkin = useCallback(async () => {
    try {
      const values = await api.getSettings();
      setSkin(isSkin(values.ui_skin) ? values.ui_skin : DEFAULT_SKIN);
    } catch {
      setSkin(DEFAULT_SKIN);
    }
  }, []);

  useEffect(() => {
    void loadSkin();
  }, [loadSkin]);

  // Look for a new version once, a few seconds in — after the library has
  // drawn, so a slow or unreachable endpoint never delays the app opening.
  // A failed check is silent here; only the button in Settings reports why.
  useEffect(() => {
    const id = window.setTimeout(() => {
      void checkForUpdate().then((found) => found && setUpdate(found));
    }, 3000);
    return () => window.clearTimeout(id);
  }, []);

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
      const result = await api.scanLibrary();
      await refreshGames();
      await refreshSidebar();

      // A new game arrives with nothing but a filename, so go and get its
      // artwork and details straight away rather than making that a second
      // thing to remember. Only when the scan actually found something, and
      // only for entries that still need it — this re-runs no work. The
      // progress toast carries its own cancel button.
      if (result.added > 0) {
        try {
          const summary = await api.scrapeLibrary(null);
          setScrape((prev) =>
            prev ? { ...prev, done: true, status: summary } : prev,
          );
        } catch (e) {
          // Every provider switched off is a setting, not a failed scan.
          setScrape((prev) =>
            prev ? { ...prev, done: true, status: api.errorMessage(e) } : prev,
          );
        }
        await refreshGames();
      }
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

  // The console and living-room skins are built around a chosen game: their
  // hero art, caption and Start button have nothing to say without one. So
  // they always have a selection, and it follows the list when filtering
  // leaves the old choice behind. The desktop skin does the opposite — there,
  // selecting is what opens the panel, so it must stay deliberate.
  useEffect(() => {
    if (skin === "launchbox" || games.length === 0) return;
    if (selectedId !== null && games.some((g) => g.id === selectedId)) return;
    setSelectedId(games[0].id);
  }, [skin, games, selectedId]);

  // ------------------------------------------------------------ controller

  const anyModalOpen = settingsOpen || homebrewOpen || statsOpen || detailOpen;

  useEffect(() => {
    document.body.classList.toggle("pad-active", padConnected);
  }, [padConnected]);

  // Land somewhere sensible when the view changes under the pad, so the first
  // press moves rather than having to first find a starting point.
  useEffect(() => {
    if (!padConnected) return;
    const active = document.activeElement as HTMLElement | null;
    if (!active || !active.hasAttribute("data-nav")) {
      const id = window.setTimeout(focusFirst, 60);
      return () => window.clearTimeout(id);
    }
  }, [padConnected, skin, games, selectedId, anyModalOpen]);

  const closeTopmost = useCallback(() => {
    if (homebrewOpen) return setHomebrewOpen(false);
    if (statsOpen) return setStatsOpen(false);
    if (settingsOpen) return setSettingsOpen(false);
    if (detailOpen) return setDetailOpen(false);
    if (skin === "launchbox" && selectedId !== null) return setSelectedId(null);
  }, [homebrewOpen, statsOpen, settingsOpen, detailOpen, skin, selectedId]);

  // Shoulder buttons page through systems, which is the one move that is
  // tedious with a stick and instant with a bumper.
  const stepPlatform = useCallback(
    (delta: number) => {
      const withGames = platforms.filter((p) => p.gameCount > 0);
      if (withGames.length === 0) return;
      const slugs: Array<string | null> = [null, ...withGames.map((p) => p.slug)];
      const at = slugs.indexOf(platform);
      const next = slugs[(at + delta + slugs.length) % slugs.length];
      setPlatform(next);
      setFavoritesOnly(false);
      setUnscrapedOnly(false);
    },
    [platforms, platform],
  );

  useGamepad({
    onConnected: setPadConnected,
    onDirection: (dir) => {
      if (!focusInDirection(dir)) focusFirst();
    },
    onConfirm: () => {
      // On a game tile, the obvious thing a pad should do is start the game;
      // anywhere else, press what is focused.
      const active = document.activeElement as HTMLElement | null;
      const id = active?.dataset.gameId;
      if (id && !anyModalOpen) {
        handleLaunch(Number(id));
        return;
      }
      activateFocused();
    },
    onBack: closeTopmost,
    onAlt: () => {
      const active = document.activeElement as HTMLElement | null;
      const raw = active?.dataset.gameId;
      const id = raw ? Number(raw) : selectedId;
      if (id !== null && id !== undefined && !anyModalOpen) {
        setSelectedId(id);
        setDetailOpen(true);
      }
    },
    onAux: () => {
      const active = document.activeElement as HTMLElement | null;
      const raw = active?.dataset.gameId;
      const id = raw ? Number(raw) : selectedId;
      const game = games.find((g) => g.id === id);
      if (game && !anyModalOpen) void handleToggleFavorite(game);
    },
    onStart: () => {
      if (selectedId !== null && !anyModalOpen) handleLaunch(selectedId);
    },
    onShoulder: (side) => {
      if (!anyModalOpen) stepPlatform(side === "left" ? -1 : 1);
    },
  });

  // ------------------------------------------------------------ rendering

  const shell: ShellProps = {
    games,
    platforms,
    stats,
    platform,
    search,
    sort,
    favoritesOnly,
    unscrapedOnly,
    selectedId,
    selected,
    busy,
    error,
    isEmptyLibrary: !!isEmptyLibrary,
    padConnected,
    onSelectPlatform: (slug) => {
      setPlatform(slug);
      setFavoritesOnly(false);
      setUnscrapedOnly(false);
    },
    onToggleFavorites: () => {
      setFavoritesOnly((v) => !v);
      setUnscrapedOnly(false);
    },
    onToggleUnscraped: () => {
      setUnscrapedOnly((v) => !v);
      setFavoritesOnly(false);
    },
    onSearch: setSearch,
    onSort: setSort,
    onSelect: setSelectedId,
    onLaunch: handleLaunch,
    onToggleFavorite: handleToggleFavorite,
    onOpenDetail: (id) => {
      setSelectedId(id);
      setDetailOpen(true);
    },
    onScan: handleScan,
    onScrape: handleScrape,
    onOpenSettings: () => setSettingsOpen(true),
    onOpenHomebrew: () => setHomebrewOpen(true),
    onOpenStats: () => setStatsOpen(true),
  };

  // On the desktop skin, picking a game *is* opening its panel. The console
  // and living-room skins show their own game page instead, so there the
  // panel is the "Options" / "Manage" button and nothing else.
  const showDetail = selected && (skin === "launchbox" || detailOpen);

  const closeDetail = () => {
    setDetailOpen(false);
    if (skin === "launchbox") setSelectedId(null);
  };

  return (
    <>
      {skin === "switch" ? (
        <SwitchShell {...shell} />
      ) : skin === "steam" ? (
        <SteamShell {...shell} />
      ) : (
        <LaunchboxShell {...shell} />
      )}

      {showDetail && selected && (
        <GameDetail
          game={selected}
          onClose={closeDetail}
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
          onSkinChange={setSkin}
          onClose={() => {
            setSettingsOpen(false);
            void loadSkin();
            void refreshSidebar();
            void refreshGames();
          }}
        />
      )}

      {statsOpen && (
        <StatsModal
          onClose={() => setStatsOpen(false)}
          onPick={(id) => {
            setStatsOpen(false);
            setSelectedId(id);
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

      {update && !updateDismissed && (
        <UpdateBanner info={update} onDismiss={() => setUpdateDismissed(true)} />
      )}

      {(scan || scrape || nowPlaying) && (
        <ProgressToast
          scan={scan}
          scrape={scrape}
          nowPlaying={nowPlaying}
          onCancelScrape={() => void api.cancelScrape()}
        />
      )}
    </>
  );
}
