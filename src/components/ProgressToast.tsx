import type { ScanProgress, ScrapeProgress } from "../types";

interface Props {
  scan: ScanProgress | null;
  scrape: ScrapeProgress | null;
  nowPlaying: string | null;
  onCancelScrape: () => void;
}

export default function ProgressToast({
  scan,
  scrape,
  nowPlaying,
  onCancelScrape,
}: Props) {
  if (nowPlaying) {
    return (
      <div className="toast">
        <div className="toast-head">
          <strong>Playing</strong>
          <span className="toast-msg">{nowPlaying}</span>
        </div>
        <div className="hint">
          Playtime is recorded when the emulator closes.
        </div>
      </div>
    );
  }

  const active = scrape ?? scan;
  if (!active) return null;

  const isScrape = active === scrape;
  const pct = active.total > 0 ? (active.current / active.total) * 100 : 0;

  const label = isScrape
    ? scrape!.done
      ? "Metadata complete"
      : "Fetching metadata"
    : scan!.done
      ? "Scan complete"
      : scan!.phase === "discovering"
        ? "Finding ROMs"
        : "Reading ROMs";

  const detail = isScrape
    ? (scrape!.haltedReason ?? scrape!.title)
    : scan!.message;

  // A finished scan explains what it threw out, so a missing game is
  // traceable rather than mysterious.
  const ignoredNote =
    !isScrape && scan!.ignored > 0
      ? `Ignored ${scan!.ignored} non-game file${scan!.ignored === 1 ? "" : "s"}` +
        (scan!.ignoredSummary ? ` — ${scan!.ignoredSummary}` : "")
      : null;

  return (
    <div className="toast">
      <div className="toast-head">
        <strong>{label}</strong>
        <span className="toast-msg">{detail}</span>
        {active.total > 0 && (
          <span className="card-sub">
            {active.current}/{active.total}
          </span>
        )}
        {isScrape && !scrape!.done && (
          <button className="btn small" onClick={onCancelScrape}>
            Stop
          </button>
        )}
      </div>
      <div className="bar">
        <div
          className="bar-fill"
          style={{ width: `${active.done ? 100 : pct}%` }}
        />
      </div>
      {ignoredNote && (
        <div className="hint" style={{ marginTop: 8 }}>
          {ignoredNote}
        </div>
      )}
    </div>
  );
}
