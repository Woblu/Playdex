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

export type SkinName =
  | "launchbox"
  | "switch"
  | "steam"
  | "arc"
  | "blades"
  | "showcase"
  | "browser";

export const SKINS: Array<{ value: SkinName; label: string; blurb: string }> = [
  {
    value: "launchbox",
    label: "Romcade",
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
  {
    value: "arc",
    label: "Arc",
    blurb:
      "One column of slabs down the middle of a dark screen, lit from one side.",
  },
  {
    value: "blades",
    label: "Blades",
    blurb:
      "Panels side by side, one open and the rest turned edge-on. The light one.",
  },
  {
    value: "showcase",
    label: "Showcase",
    blurb:
      "A strip of tiles along the top, and whatever is selected spread out below.",
  },
  {
    value: "browser",
    label: "Browser",
    blurb: "A column of cards standing in a slow-drifting field. Unhurried.",
  },
];

export const DEFAULT_SKIN: SkinName = "launchbox";

export function isSkin(value: string | undefined): value is SkinName {
  return SKINS.some((skin) => skin.value === value);
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

  /**
   * Let a skin claim the Back button for its own internal state.
   *
   * A skin can be somewhere the app knows nothing about — the console skin's
   * "All software" grid is a view inside the skin, not a panel App opened —
   * and Back should leave it before it starts closing anything else. Register
   * a handler that returns true when it dealt with the press, or null to stop
   * claiming it.
   */
  registerBack: (handler: (() => boolean) | null) => void;

  onScan: () => void;
  onScrape: () => void;
  onOpenSettings: () => void;
  onOpenStats: () => void;
}
