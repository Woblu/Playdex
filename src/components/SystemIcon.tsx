/**
 * Original line-art glyphs for each system family.
 *
 * These are drawn rather than sourced from console logos, which are
 * trademarked. Grouping by physical media — cartridge, handheld, disc, arcade
 * cabinet, home computer, floppy — also reads faster in a list than three dozen
 * near-identical logos would.
 */

type Kind =
  | "cartridge"
  | "handheld"
  | "disc"
  | "arcade"
  | "computer"
  | "floppy"
  | "all"
  | "star"
  | "missing"
  | "chip";

const FAMILY: Record<string, Kind> = {
  // Cartridge-based home consoles
  nes: "cartridge",
  snes: "cartridge",
  n64: "cartridge",
  genesis: "cartridge",
  sms: "cartridge",
  sega32x: "cartridge",
  pcengine: "cartridge",
  atari2600: "cartridge",
  atari7800: "cartridge",
  jaguar: "cartridge",
  colecovision: "cartridge",
  intellivision: "cartridge",
  virtualboy: "cartridge",

  // Handhelds
  gb: "handheld",
  gbc: "handheld",
  gba: "handheld",
  nds: "handheld",
  n3ds: "handheld",
  gamegear: "handheld",
  lynx: "handheld",
  wonderswan: "handheld",
  ngp: "handheld",
  psp: "handheld",

  // Disc-based
  ps1: "disc",
  ps2: "disc",
  saturn: "disc",
  dreamcast: "disc",
  segacd: "disc",
  threedo: "disc",
  gamecube: "disc",
  wii: "disc",

  // Coin-op
  arcade: "arcade",
  neogeo: "arcade",

  // Home computers
  c64: "computer",
  amiga: "computer",
  msx: "computer",

  // Disk-based PC
  dos: "floppy",
  scummvm: "floppy",
};

export function familyFor(platform: string): Kind {
  return FAMILY[platform] ?? "chip";
}

const PATHS: Record<Kind, React.ReactNode> = {
  cartridge: (
    <>
      <path d="M6 21V5a2 2 0 0 1 2-2h5.6a2 2 0 0 1 1.4.6l2.4 2.4a2 2 0 0 1 .6 1.4V21" />
      <rect x="9" y="6" width="6" height="4" rx="0.6" />
      <path d="M9 21v-2.5M12 21v-2.5M15 21v-2.5" />
    </>
  ),
  handheld: (
    <>
      <rect x="6" y="2.5" width="12" height="19" rx="2.2" />
      <rect x="8.5" y="5" width="7" height="6" rx="0.6" />
      <path d="M9 15.5h2.4M10.2 14.3v2.4" />
      <circle cx="15" cy="15.5" r="0.9" />
    </>
  ),
  disc: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <circle cx="12" cy="12" r="2.4" />
      <path d="M12 3.5a8.5 8.5 0 0 1 7.4 4.3" />
    </>
  ),
  arcade: (
    <>
      <path d="M6 21V6a3 3 0 0 1 3-3h6a3 3 0 0 1 3 3v15" />
      <rect x="8.5" y="6" width="7" height="5" rx="0.6" />
      <path d="M8.5 14.5h7M6 21h12" />
      <circle cx="10.2" cy="17.6" r="0.8" />
      <circle cx="13.8" cy="17.6" r="0.8" />
    </>
  ),
  computer: (
    <>
      <rect x="3" y="4" width="18" height="11" rx="1.6" />
      <path d="M9 19h6M12 15v4" />
      <path d="M6.5 7.5h5" />
    </>
  ),
  floppy: (
    <>
      <path d="M4 5.5A1.5 1.5 0 0 1 5.5 4h10l4.5 4.5v10a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18.5Z" />
      <path d="M8 4v5h6V4" />
      <rect x="7.5" y="13" width="9" height="7" rx="0.6" />
    </>
  ),
  all: (
    <>
      <rect x="3.5" y="3.5" width="7" height="7" rx="1.2" />
      <rect x="13.5" y="3.5" width="7" height="7" rx="1.2" />
      <rect x="3.5" y="13.5" width="7" height="7" rx="1.2" />
      <rect x="13.5" y="13.5" width="7" height="7" rx="1.2" />
    </>
  ),
  star: (
    <path d="m12 3.6 2.6 5.3 5.8.85-4.2 4.1 1 5.8-5.2-2.73L6.8 19.65l1-5.8-4.2-4.1 5.8-.85Z" />
  ),
  missing: (
    <>
      <rect x="3.5" y="5" width="17" height="14" rx="2" />
      <path d="M8.6 12h6.8" />
    </>
  ),
  chip: (
    <>
      <rect x="6.5" y="6.5" width="11" height="11" rx="1.6" />
      <path d="M10 3.5v3M14 3.5v3M10 17.5v3M14 17.5v3M3.5 10h3M3.5 14h3M17.5 10h3M17.5 14h3" />
    </>
  ),
};

interface Props {
  /** A platform slug, or one of the sidebar pseudo-icons. */
  platform: string;
  size?: number;
  className?: string;
}

export default function SystemIcon({ platform, size = 16, className }: Props) {
  const kind: Kind =
    platform === "all" || platform === "star" || platform === "missing"
      ? (platform as Kind)
      : familyFor(platform);

  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {PATHS[kind]}
    </svg>
  );
}
