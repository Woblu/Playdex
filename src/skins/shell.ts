/**
 * What every skin is handed.
 *
 * A skin decides only how the library *looks* and how you move through it.
 * All state, fetching and side effects stay in `App`, so the three of them
 * cannot drift apart in behaviour — only in appearance. Anything a skin does
 * not draw (cheats, saves, ROM hacks) is still reachable through the shared
 * detail panel, which is why `onOpenDetail` exists separately from
 * `onSelect`.
 */

import type { Game, LibraryStats, PlatformInfo, SortKey } from "../types";

export type SkinName = "launchbox" | "switch" | "steam";

export const SKINS: Array<{ value: SkinName; label: string; blurb: string }> = [
  {
    value: "launchbox",
    label: "Playdex",
    blurb: "A dense desktop library: sidebar, sortable grid, detail panel.",
  },
  {
    value: "switch",
    label: "Console",
    blurb:
      "A handheld home screen — one big row of square icons, driven by the D-pad.",
  },
  {
    value: "steam",
    label: "Big Picture",
    blurb: "Living-room layout: hero art up top, capsules below, made for a pad.",
  },
];

export const DEFAULT_SKIN: SkinName = "launchbox";

export function isSkin(value: string | undefined): value is SkinName {
  return value === "launchbox" || value === "switch" || value === "steam";
}

export interface ShellProps {
  games: Game[];
  platforms: PlatformInfo[];
  stats: LibraryStats | null;

  platform: string | null;
  search: string;
  sort: SortKey;
  favoritesOnly: boolean;
  unscrapedOnly: boolean;

  selectedId: number | null;
  selected: Game | null;
  busy: boolean;
  error: string | null;
  isEmptyLibrary: boolean;
  /** True while a pad is connected, so skins can show button hints. */
  padConnected: boolean;

  onSelectPlatform: (slug: string | null) => void;
  onToggleFavorites: () => void;
  onToggleUnscraped: () => void;
  onSearch: (value: string) => void;
  onSort: (value: SortKey) => void;

  onSelect: (id: number) => void;
  onLaunch: (id: number) => void;
  onToggleFavorite: (game: Game) => void;
  /** Open the full detail panel — everything a skin does not draw itself. */
  onOpenDetail: (id: number) => void;

  onScan: () => void;
  onScrape: () => void;
  onOpenSettings: () => void;
  onOpenStats: () => void;
}
