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
 *    map of what is next to what. Anything carrying `data-nav` joins in, so a
 *    new skin or a new button is navigable the moment it renders and there is
 *    no second structure to keep in sync with the markup.
 */

import { useEffect, useRef } from "react";

export type NavDirection = "up" | "down" | "left" | "right";

export interface GamepadHandlers {
  onDirection?: (dir: NavDirection) => void;
  /** Bottom face button: activate whatever is focused. */
  onConfirm?: () => void;
  /** Right face button: close, or step back. */
  onBack?: () => void;
  /** Left face button. */
  onAlt?: () => void;
  /** Top face button. */
  onAux?: () => void;
  onStart?: () => void;
  onSelect?: () => void;
  onShoulder?: (side: "left" | "right") => void;
  /** Fires when a pad is first seen, so the UI can show button hints. */
  onConnected?: (connected: boolean) => void;
}

/** Standard-mapping button indices, named so the wiring below reads. */
const BTN = {
  confirm: 0,
  back: 1,
  alt: 2,
  aux: 3,
  lb: 4,
  rb: 5,
  select: 8,
  start: 9,
  up: 12,
  down: 13,
  left: 14,
  right: 15,
} as const;

/** Past this a stick counts as pushed; below 60% of it, released again. */
const STICK = 0.55;

/** Held-direction repeat, in milliseconds: first wait, then the rate. */
const REPEAT_DELAY = 420;
const REPEAT_RATE = 110;

export function useGamepad(handlers: GamepadHandlers, enabled = true) {
  // Kept in a ref so changing handlers never restarts the polling loop.
  const ref = useRef(handlers);
  ref.current = handlers;

  useEffect(() => {
    if (!enabled) return;
    if (typeof navigator === "undefined" || !navigator.getGamepads) return;

    let raf = 0;
    let sawPad = false;
    const held = new Map<number, boolean>();
    // Per-direction: when it may next fire.
    const nextRepeat = new Map<NavDirection, number>();

    const pressed = (pad: Gamepad, index: number) =>
      pad.buttons.length > index && pad.buttons[index].pressed;

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
          nextRepeat.clear();
          ref.current.onConnected?.(false);
        }
        return;
      }
      if (!sawPad) {
        sawPad = true;
        ref.current.onConnected?.(true);
      }

      const now = performance.now();
      const x = pad.axes[0] ?? 0;
      const y = pad.axes[1] ?? 0;

      direction("left", pressed(pad, BTN.left) || x < -STICK, now);
      direction("right", pressed(pad, BTN.right) || x > STICK, now);
      direction("up", pressed(pad, BTN.up) || y < -STICK, now);
      direction("down", pressed(pad, BTN.down) || y > STICK, now);

      if (edge(pad, BTN.confirm)) ref.current.onConfirm?.();
      if (edge(pad, BTN.back)) ref.current.onBack?.();
      if (edge(pad, BTN.alt)) ref.current.onAlt?.();
      if (edge(pad, BTN.aux)) ref.current.onAux?.();
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
 * controller and reachable with Tab — a difference nobody would notice until
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
      // Off-screen inside a scroll container still counts — moving there
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
 * weighted far more heavily than drift across it — so "down" from a grid cell
 * prefers the cell directly below over a nearer one two columns away, which
 * is the behaviour that makes a grid feel like a grid.
 */
export function focusInDirection(dir: NavDirection): boolean {
  const items = candidates();
  if (items.length === 0) return false;

  const active = document.activeElement as HTMLElement | null;
  if (!active || !items.includes(active)) {
    items[0].focus();
    scrollIntoViewIfNeeded(items[0]);
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
  best.focus();
  scrollIntoViewIfNeeded(best);
  return true;
}

function scrollIntoViewIfNeeded(el: HTMLElement) {
  el.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
}

/**
 * Activate whatever is focused, as a click would — except a text field, where
 * the button should not retype the last thing that happened to be there.
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
  (preferred ?? items[0])?.focus();
}
