import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type {
  EmulatorConfig,
  Game,
  HackPreview,
  GameFilter,
  LibraryFolder,
  LibraryInsights,
  LibraryStats,
  Cheat,
  CacheUsage,
  DropTally,
  ScanProgress,
  DetectedEmulator,
  HackBundle,
  PatchEntry,
  PlatformInfo,
  RetroArchCheats,
  SaveEntry,
  ProviderStatus,
  Settings,
} from "./types";

// ------------------------------------------------------------- library

export const listGames = (filter: GameFilter) =>
  invoke<Game[]>("list_games", { filter });

export const getGame = (id: number) => invoke<Game | null>("get_game", { id });

export const listPlatforms = () => invoke<PlatformInfo[]>("list_platforms");

export const knownPlatforms = () => invoke<PlatformInfo[]>("known_platforms");

export const libraryStats = () => invoke<LibraryStats>("library_stats");

export const libraryInsights = () =>
  invoke<LibraryInsights>("library_insights");

export const setFavorite = (id: number, value: boolean) =>
  invoke<void>("set_favorite", { id, value });

export const setHidden = (id: number, value: boolean) =>
  invoke<void>("set_hidden", { id, value });

export const setGamePlatform = (id: number, platform: string) =>
  invoke<void>("set_game_platform", { id, platform });

export const removeGame = (id: number) => invoke<void>("remove_game", { id });

// ------------------------------------------------------------- folders

export const listLibraryFolders = () =>
  invoke<LibraryFolder[]>("list_library_folders");

export const addLibraryFolder = (path: string, platform: string | null) =>
  invoke<void>("add_library_folder", { path, platform });

export const removeLibraryFolder = (id: number) =>
  invoke<void>("remove_library_folder", { id });

export const pickFolder = () => invoke<string | null>("pick_folder");

export const pickFile = () => invoke<string | null>("pick_file");

// ---------------------------------------------------------------- scan

export const scanLibrary = () => invoke<ScanProgress>("scan_library");

export const cleanMissing = () => invoke<number>("clean_missing");

// -------------------------------------------------------------- scrape

export const scrapeLibrary = (platform: string | null) =>
  invoke<string>("scrape_library", { platform });

export const scrapeOne = (id: number) => invoke<string>("scrape_one", { id });

export const cancelScrape = () => invoke<void>("cancel_scrape");

// -------------------------------------------------------------- launch

export const launchGame = (id: number) => invoke<void>("launch_game", { id });

export const previewLaunch = (id: number) =>
  invoke<string>("preview_launch", { id });

export const revealGame = (id: number) => invoke<void>("reveal_game", { id });

// --------------------------------------------------------------- hacks

export const inspectPatch = (baseGameId: number, patchPath: string) =>
  invoke<HackPreview>("inspect_patch", { baseGameId, patchPath });

export const addHack = (
  baseGameId: number,
  patchPath: string,
  title: string | null,
) => invoke<number>("add_hack", { baseGameId, patchPath, title });

export const applyCatalogPatch = (
  baseGameId: number,
  patchId: number,
  title: string | null,
) => invoke<number>("apply_catalog_patch", { baseGameId, patchId, title });

// -------------------------------------------------------- patch catalog

export const importPatches = (path: string) =>
  invoke<string>("import_patches", { path });

export const patchesForGame = (gameId: number) =>
  invoke<PatchEntry[]>("patches_for_game", { gameId });

export const listPatches = (search: string | null) =>
  invoke<PatchEntry[]>("list_patches", { search });

export const patchCatalogSize = () => invoke<number>("patch_catalog_size");

export const clearPatchCatalog = () => invoke<void>("clear_patch_catalog");

export const findCheats = (gameId: number) =>
  invoke<Cheat[]>("find_cheats", { gameId });

export const listCheats = (gameId: number) =>
  invoke<Cheat[]>("list_cheats", { gameId });

export const setCheat = (gameId: number, index: number, enabled: boolean) =>
  invoke<void>("set_cheat", { gameId, index, enabled });

export const setAllCheats = (gameId: number, enabled: boolean) =>
  invoke<Cheat[]>("set_all_cheats", { gameId, enabled });

export const saveCheats = (gameId: number) =>
  invoke<string>("save_cheats", { gameId });

export const retroarchCheatStatus = () =>
  invoke<RetroArchCheats>("retroarch_cheat_status");

export const enableAutoApplyCheats = () =>
  invoke<string>("enable_auto_apply_cheats");

// ------------------------------------------------------- hack bundles

export const listHackBundles = () =>
  invoke<HackBundle[]>("list_hack_bundles");

export const downloadHackBundle = (bundle: HackBundle) =>
  invoke<string>("download_hack_bundle", { bundle });

// --------------------------------------------------------------- saves

export const listSaves = (gameId: number) =>
  invoke<SaveEntry[]>("list_saves", { gameId });

export const backUpSaves = (gameId: number) =>
  invoke<string>("back_up_saves", { gameId });

export const deleteSaveState = (path: string) =>
  invoke<void>("delete_save_state", { path });

// --------------------------------------------------------- diagnostics

export const detectRetroarch = () =>
  invoke<DetectedEmulator | null>("detect_retroarch");

export const testCredentials = () =>
  invoke<ProviderStatus[]>("test_credentials");

// ------------------------------------------------------------ settings

export const getSettings = () => invoke<Settings>("get_settings");

export const saveSettings = (values: Settings) =>
  invoke<void>("save_settings", { values });

export const listEmulators = () => invoke<EmulatorConfig[]>("list_emulators");

export const saveEmulator = (config: EmulatorConfig) =>
  invoke<void>("save_emulator", { config });

export const effectiveEmulator = (platform: string) =>
  invoke<EmulatorConfig>("effective_emulator", { platform });

// --------------------------------------------------------------- media

/** Cached artwork is served over our own `media://` protocol. */
export function artUrl(path: string | null): string | null {
  if (!path) return null;
  return convertFileSrc(path, "media");
}

// --------------------------------------------------------------- utils

export function formatPlaytime(seconds: number): string {
  if (!seconds) return "Never played";
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

export function formatSize(bytes: number): string {
  if (!bytes) return "—";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

export function formatDate(timestamp: number | null): string {
  if (!timestamp) return "—";
  return new Date(timestamp * 1000).toLocaleDateString();
}

/** Display name for a platform slug, falling back to the slug itself. */
export function platformName(
  platforms: PlatformInfo[],
  slug: string,
): string {
  return platforms.find((p) => p.slug === slug)?.name ?? slug;
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

// ------------------------------------------------------- unpacked cache

export const cacheUsage = () => invoke<CacheUsage>("cache_usage");
export const clearCache = () => invoke<number>("clear_cache");

export const addDropped = (paths: string[]) =>
  invoke<DropTally>("add_dropped", { paths });

export const unpackInPlace = (id: number) =>
  invoke<string>("unpack_in_place", { id });
