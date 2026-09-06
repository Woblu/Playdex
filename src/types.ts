export interface Game {
  id: number;
  path: string;
  filename: string;
  platform: string;
  size: number;
  crc32: string | null;
  md5: string | null;
  sha1: string | null;
  innerName: string | null;
  title: string;
  description: string | null;
  developer: string | null;
  publisher: string | null;
  genre: string | null;
  releaseDate: string | null;
  players: string | null;
  rating: number | null;
  region: string | null;
  coverPath: string | null;
  screenshotPath: string | null;
  logoPath: string | null;
  scrapeStatus: "pending" | "ok" | "notfound" | "error";
  scrapeSource: string | null;
  favorite: boolean;
  hidden: boolean;
  playCount: number;
  playSeconds: number;
  lastPlayed: number | null;
  addedAt: number;
  /** Set when this entry is a ROM hack built from another game's ROM. */
  baseGameId: number | null;
  patchPath: string | null;
}

export interface HackPreview {
  format: string;
  suggestedTitle: string;
  expectedCrc: string | null;
  baseCrc: string | null;
  compatible: boolean;
  message: string;
}

export interface PlatformInfo {
  slug: string;
  name: string;
  extensions: string[];
  cores: string[];
  gameCount: number;
}

export interface LibraryFolder {
  id: number;
  path: string;
  platformOverride: string | null;
  addedAt: number;
}

export interface LibraryStats {
  totalGames: number;
  totalPlatforms: number;
  scraped: number;
  totalPlaySeconds: number;
}

export interface GameFilter {
  platform?: string | null;
  search?: string | null;
  favoritesOnly?: boolean;
  unscrapedOnly?: boolean;
  showHidden?: boolean;
  sort?: SortKey;
}

export type SortKey = "title" | "recent" | "played" | "added" | "rating";

export interface ScanProgress {
  phase: string;
  current: number;
  total: number;
  message: string;
  added: number;
  skipped: number;
  /** Entries already present that were re-detected and re-hashed. */
  corrected: number;
  /** Entries dropped because they are no longer recognised as games. */
  dropped: number;
  /** Files that turned out not to be games at all. */
  ignored: number;
  /** Why they were ignored, grouped by reason. */
  ignoredSummary: string;
  done: boolean;
}

export interface ScrapeProgress {
  current: number;
  total: number;
  gameId: number;
  title: string;
  status: string;
  source: string | null;
  done: boolean;
  haltedReason: string | null;
}

export interface EmulatorConfig {
  platform: string;
  mode: "retroarch" | "custom";
  core: string | null;
  customCommand: string | null;
}

export type Settings = Record<string, string>;

export interface PatchEntry {
  id: number;
  path: string;
  name: string;
  format: string;
  sourceCrc: string | null;
  systemHint: string | null;
  targetHint: string | null;
  origin: string;
  addedAt: number;
}

export interface ImportProgress {
  scanned: number;
  imported: number;
  skipped: number;
  message: string;
  done: boolean;
}

export interface HomebrewItem {
  identifier: string;
  title: string;
  creator: string | null;
  description: string | null;
  year: string | null;
  license: string | null;
  collection: string;
  platform: string | null;
}

export interface HomebrewFile {
  name: string;
  size: number;
  url: string;
}

export interface HomebrewDetail {
  files: HomebrewFile[];
  imageUrl: string | null;
}

export interface DetectedEmulator {
  path: string;
  coresDir: string | null;
  source: string;
  coreCount: number;
}

export interface ProviderStatus {
  provider: string;
  configured: boolean;
  ok: boolean;
  message: string;
  quota: string | null;
}

export interface Cheat {
  index: number;
  description: string;
  /** A Game Genie code, or several joined with "+". */
  code: string;
  enabled: boolean;
}

export interface HackBundle {
  name: string;
  size: number;
  url: string;
}

export interface RetroArchCheats {
  configPath: string | null;
  cheatDir: string | null;
  /** RetroArch only applies a cheat file on its own when this is true. */
  autoApply: boolean;
}

export interface SaveEntry {
  /** "save" for battery progress, "state" for an emulator snapshot. */
  kind: string;
  slot: number | null;
  name: string;
  path: string;
  size: number;
  modified: number;
  screenshot: string | null;
}

export interface LibraryInsights {
  totalGames: number;
  gamesPlayed: number;
  totalPlaySeconds: number;
  sessionCount: number;
  longestSession: number;
  recent: Game[];
  mostPlayed: Game[];
}

export interface CacheUsage {
  bytes: number;
  entries: number;
  limitBytes: number;
}
