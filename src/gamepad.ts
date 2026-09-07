/**
 * Controller support.
 *
 * Two halves that stay deliberately separate:
 *
 *  - `useGamepad` turns the browser's polling Gamepad API into discrete
 *    events. Pads report *state*, not presses, so every frame is compared
 *    with the last to find the edges, and held directions repeat on a
 *    keyboard-like delay so a menu can be scrolled without machine-gunning.
 *
 *  - `focusInDirection` moves focus by geometry rather than by a hand-written
 *    map of what is next to what. Anything ordinarily interactive joins in, so
 *    a new skin or a new button is navigable the moment it renders and there
 *    is no second structure to keep in sync with the markup.
 */

import { useEffect, useRef } from "react";

export type NavDirection = "up" | "down" | "left" | "right";

export interface PadInfo {
  id: string;
  /** True when the browser recognised the pad and normalised its layout. */
  standard: boolean;
  buttons: number;
  axes: number;
}

export interface GamepadHandlers {
  onDirection?: (dir: NavDirection) => void;
  /** Activate whatever is focused. */
  onConfirm?: () => void;
  /** Close, or step back. */
  onBack?: () => void;
  onAlt?: () => void;
  onAux?: () => void;
  onStart?: () => void;
  onSelect?: () => void;
  onShoulder?: (side: "left" | "right") => void;
  /** Fires when a pad appears or goes away, so the UI can show button hints. */
  onConnected?: (connected: boolean, info: PadInfo | null) => void;
  /** Every button press by its raw index, for the setup screen. */
  onRawButton?: (index: number) => void;
}

/**
 * Which physical button sits at which index.
 *
 * The Gamepad API only promises these positions when it recognises the pad and
 * reports `mapping: "standard"`. Anything it has not seen before, and a pad
 * released last week is exactly that, arrives in raw HID order where the
 * indices mean whatever the manufacturer decided. Nintendo pads also swap the
 * face buttons relative to Xbox, so even a recognised one wants A and B the
 * other way round.
 *
 * So button positions are configuration, not a constant. `custom` exists
 * because no amount of guessing beats letting someone press the button they
 * mean.
 */
export interface PadBindings {
  confirm: number;
  back: number;
  alt: number;
  aux: number;
}

export type PadLayout = "auto" | "standard" | "nintendo" | "custom";

export const LAYOUTS: Record<"standard" | "nintendo", PadBindings> = {
  // Xbox-style: A is the bottom face button.
  standard: { confirm: 0, back: 1, alt: 2, aux: 3 },
  // Nintendo-style: A on the right, B at the bottom, X and Y swapped too.
  nintendo: { confirm: 1, back: 0, alt: 3, aux: 2 },
};

/** Shoulders, sticks and D-pad, which layouts agree on where they exist. */
const BTN = {
  lb: 4,
  rb: 5,
  select: 8,
  start: 9,
  up: 12,
  down: 13,
  left: 14,
  right: 15,
} as const;

/** Past this a stick counts as pushed. */
const STICK = 0.55;

/** Held-direction repeat, in milliseconds: first wait, then the rate. */
const REPEAT_DELAY = 420;
const REPEAT_RATE = 110;

/** Guess a layout from what the pad calls itself. */
export function detectLayout(info: PadInfo | null): "standard" | "nintendo" {
  if (!info) return "standard";
  const id = info.id.toLowerCase();
  const nintendo =
    id.includes("nintendo") ||
    id.includes("switch") ||
    id.includes("joy-con") ||
    id.includes("joycon") ||
    id.includes("pro controller");
  return nintendo ? "nintendo" : "standard";
}

export function resolveBindings(
  layout: PadLayout,
  custom: PadBindings | null,
  info: PadInfo | null,
): PadBindings {
  if (layout === "custom" && custom) return custom;
  if (layout === "standard" || layout === "nintendo") return LAYOUTS[layout];
  return LAYOUTS[detectLayout(info)];
}

/**
 * On an unrecognised pad the D-pad is usually a hat switch on an axis rather
 * than four buttons. It reports eight directions as fractions of a turn, so
 * the angle gets decoded instead of matching a magic list of values.
 */
function hatDirections(value: number): Record<NavDirection, boolean> {
  const none = { up: false, down: false, left: false, right: false };
  // Resting is reported outside the range, commonly 1.28 or -1.
  if (value < -1.05 || value > 1.05) return none;

  const turn = ((value + 1) / 2) * 360; // 0 = up, going clockwise
  const near = (deg: number) => {
    const away = Math.abs(((turn - deg + 540) % 360) - 180);
    return 180 - away <= 67;
  };
  return { up: near(0), right: near(90), down: near(180), left: near(270) };
}

export function useGamepad(
  handlers: GamepadHandlers,
  opts: { enabled?: boolean; bindings?: PadBindings } = {},
) {
  // Kept in refs so changing either never restarts the polling loop.
  const ref = useRef(handlers);
  ref.current = handlers;
  const bindingsRef = useRef<PadBindings>(opts.bindings ?? LAYOUTS.standard);
  bindingsRef.current = opts.bindings ?? LAYOUTS.standard;

  const enabled = opts.enabled ?? true;

  useEffect(() => {
    if (!enabled) return;
    if (typeof navigator === "undefined" || !navigator.getGamepads) return;

    let raf = 0;
    let sawPad = false;
    /** Last frame's state, keyed by button index. */
    const held = new Map<number, boolean>();
    /** Separate keys for the raw listener, so both can fire in one frame. */
    const heldRaw = new Map<number, boolean>();
    const nextRepeat = new Map<NavDirection, number>();

    const pressed = (pad: Gamepad, index: number) =>
      index >= 0 && pad.buttons.length > index && pad.buttons[index].pressed;

    const edge = (pad: Gamepad, index: number): boolean => {
      const now = pressed(pad, index);
      const before = held.get(index) ?? false;
      held.set(index, now);
      return now && !before;
    };

    const direction = (dir: NavDirection, active: boolean, now: number) => {
      if (!active) {
        nextRepeat.delete(dir);
        return;
      }
      const due = nextRepeat.get(dir);
      if (due === undefined) {
        nextRepeat.set(dir, now + REPEAT_DELAY);
        ref.current.onDirection?.(dir);
      } else if (now >= due) {
        nextRepeat.set(dir, now + REPEAT_RATE);
        ref.current.onDirection?.(dir);
      }
    };

    const poll = () => {
      raf = requestAnimationFrame(poll);

      const pads = navigator.getGamepads?.() ?? [];
      const pad = Array.from(pads).find((p): p is Gamepad => !!p && p.connected);

      if (!pad) {
        if (sawPad) {
          sawPad = false;
          held.clear();
          heldRaw.clear();
          nextRepeat.clear();
          ref.current.onConnected?.(false, null);
        }
        return;
      }

      if (!sawPad) {
        sawPad = true;
        ref.current.onConnected?.(true, {
          id: pad.id,
          standard: pad.mapping === "standard",
          buttons: pad.buttons.length,
          axes: pad.axes.length,
        });
      }

      // Report presses by raw index, so the setup screen can show what the pad
      // actually sends rather than leaving someone to guess.
      if (ref.current.onRawButton) {
        for (let i = 0; i < pad.buttons.length; i++) {
          const down = pad.buttons[i].pressed;
          if (down && !(heldRaw.get(i) ?? false)) ref.current.onRawButton(i);
          heldRaw.set(i, down);
        }
      }

      const now = performance.now();
      const b = bindingsRef.current;
      const x = pad.axes[0] ?? 0;
      const y = pad.axes[1] ?? 0;

      // A recognised pad puts the D-pad on buttons; an unrecognised one
      // usually puts it on a hat axis instead.
      const hasDpadButtons = pad.buttons.length > BTN.right;
      const hat =
        !hasDpadButtons && pad.axes.length > 9
          ? hatDirections(pad.axes[9])
          : { up: false, down: false, left: false, right: false };

      direction("left", pressed(pad, BTN.left) || hat.left || x < -STICK, now);
      direction("right", pressed(pad, BTN.right) || hat.right || x > STICK, now);
      direction("up", pressed(pad, BTN.up) || hat.up || y < -STICK, now);
      direction("down", pressed(pad, BTN.down) || hat.down || y > STICK, now);

      if (edge(pad, b.confirm)) ref.current.onConfirm?.();
      if (edge(pad, b.back)) ref.current.onBack?.();
      if (edge(pad, b.alt)) ref.current.onAlt?.();
      if (edge(pad, b.aux)) ref.current.onAux?.();
      if (edge(pad, BTN.start)) ref.current.onStart?.();
      if (edge(pad, BTN.select)) ref.current.onSelect?.();
      if (edge(pad, BTN.lb)) ref.current.onShoulder?.("left");
      if (edge(pad, BTN.rb)) ref.current.onShoulder?.("right");
    };

    raf = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(raf);
  }, [enabled]);
}

// --------------------------------------------------------------- focus

/**
 * Everything ordinarily interactive counts, rather than only what has been
 * marked up for the pad. Tagging each control by hand would mean every new
 * button is one forgotten attribute away from being unreachable with a
 * controller and reachable with Tab, a difference nobody would notice until
 * they were holding the pad. `data-nav` is still honoured, for marking
 * something that is not natively focusable.
 */
const FOCUSABLE = [
  "button:not([disabled])",
  "input:not([disabled]):not([type=hidden])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "a[href]",
  "[data-nav]",
].join(",");

/**
 * The elements a controller may land on, narrowed to the topmost overlay when
 * one is open so a dialog cannot be navigated out from underneath.
 */
function candidates(): HTMLElement[] {
  const overlays = document.querySelectorAll<HTMLElement>(".overlay, .detail");
  const scope: ParentNode = overlays.length
    ? overlays[overlays.length - 1]
    : document;

  return Array.from(scope.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => {
      if (el.hasAttribute("disabled")) return false;
      if (el.dataset.nav === "off") return false;
      const rect = el.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) return false;
      // Off-screen inside a scroll container still counts, since moving there
      // scrolls it in. Genuinely hidden does not.
      return el.offsetParent !== null || getComputedStyle(el).position === "fixed";
    },
  );
}

function centre(el: Element) {
  const r = el.getBoundingClientRect();
  return { x: r.left + r.width / 2, y: r.top + r.height / 2, r };
}

/**
 * Move focus one step in `dir`.
 *
 * Candidates are scored by distance, with movement along the axis of travel
 * weighted far more heavily than drift across it, so "down" from a grid cell
 * prefers the cell directly below over a nearer one two columns away. That is
 * the behaviour that makes a grid feel like a grid.
 */
export function focusInDirection(dir: NavDirection): boolean {
  const items = candidates();
  if (items.length === 0) return false;

  const active = document.activeElement as HTMLElement | null;
  if (!active || !items.includes(active)) {
    focusQuietly(items[0]);
    return true;
  }

  const from = centre(active);
  const horizontal = dir === "left" || dir === "right";
  const sign = dir === "right" || dir === "down" ? 1 : -1;

  let best: HTMLElement | null = null;
  let bestScore = Infinity;

  for (const el of items) {
    if (el === active) continue;
    const to = centre(el);

    const along = horizontal ? (to.x - from.x) * sign : (to.y - from.y) * sign;
    const across = horizontal ? to.y - from.y : to.x - from.x;

    // Must actually lie in the direction asked for. The tolerance forgives
    // rows whose items are not pixel-aligned.
    if (along < 6) continue;

    const score = along + Math.abs(across) * 3;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }

  if (!best) return false;
  focusQuietly(best);
  return true;
}

/**
 * The nearest ancestor that is genuinely meant to scroll.
 *
 * `overflow: hidden` is excluded deliberately. It is still scrollable through
 * script, so anything that overflows a clipped box can be dragged into view by
 * the browser, taking the whole layout with it - which is a bug every time,
 * never what was wanted.
 */
function scrollParent(el: HTMLElement): HTMLElement | null {
  let node = el.parentElement;
  while (node) {
    const style = getComputedStyle(node);
    if (
      (/(auto|scroll)/.test(style.overflowX) && node.scrollWidth > node.clientWidth) ||
      (/(auto|scroll)/.test(style.overflowY) && node.scrollHeight > node.clientHeight)
    ) {
      return node;
    }
    node = node.parentElement;
  }
  return null;
}

/**
 * Bring an element into view by scrolling only the box that is supposed to
 * scroll, rather than letting the browser walk up the tree scrolling whatever
 * it finds.
 */
export function bringIntoView(el: HTMLElement) {
  const parent = scrollParent(el);
  if (!parent) return;

  const box = parent.getBoundingClientRect();
  const rect = el.getBoundingClientRect();
  const margin = 24;

  if (rect.left < box.left) {
    parent.scrollLeft += rect.left - box.left - margin;
  } else if (rect.right > box.right) {
    parent.scrollLeft += rect.right - box.right + margin;
  }

  if (rect.top < box.top) {
    parent.scrollTop += rect.top - box.top - margin;
  } else if (rect.bottom > box.bottom) {
    parent.scrollTop += rect.bottom - box.bottom + margin;
  }
}

/** Focus without letting the browser do its own scrolling on the way. */
function focusQuietly(el: HTMLElement) {
  el.focus({ preventScroll: true });
  bringIntoView(el);
}

/**
 * Activate whatever is focused, as a click would, except a text field where
 * the button should not retype whatever happened to be there.
 */
export function activateFocused(): boolean {
  const active = document.activeElement as HTMLElement | null;
  if (!active) return false;
  if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) {
    return false;
  }
  active.click();
  return true;
}

/** Put focus somewhere sensible when a view first appears. */
export function focusFirst(): void {
  const items = candidates();
  const preferred = items.find((el) => el.dataset.navDefault !== undefined);
  const target = preferred ?? items[0];
  if (target) focusQuietly(target);
}
