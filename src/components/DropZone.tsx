/**
 * Drag a ROM onto the window and it joins the library.
 *
 * The whole window is the target rather than a marked-out rectangle. Someone
 * dragging a file at a program is not aiming at anything in particular, and a
 * small target is a thing to miss.
 *
 * Tauri does the dragging itself rather than the webview: with drag-and-drop
 * enabled the HTML5 events never fire, and instead the window reports paths on
 * disk. That is the better deal here - a path can be hashed and indexed where
 * it lies, where a browser `File` would have to be read through the webview
 * first, and these are often gigabytes.
 */

import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

interface Props {
  /** Called with the dropped paths; resolves to a line to show. */
  onDrop: (paths: string[]) => Promise<void>;
}

export default function DropZone({ onDrop }: Props) {
  const [over, setOver] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const pending = getCurrentWebview().onDragDropEvent(async (event) => {
      if (cancelled) return;

      if (event.payload.type === "over") {
        setOver(true);
      } else if (event.payload.type === "drop") {
        setOver(false);
        const paths = event.payload.paths ?? [];
        if (paths.length === 0) return;
        setBusy(true);
        try {
          await onDrop(paths);
        } finally {
          if (!cancelled) setBusy(false);
        }
      } else {
        setOver(false);
      }
    });

    return () => {
      cancelled = true;
      void pending.then((unlisten) => unlisten());
    };
    // `onDrop` is recreated every render by the caller; re-subscribing on each
    // one would drop events mid-drag, so the handler is bound once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!over && !busy) return null;

  return (
    <div className="dropzone" aria-hidden="true">
      <div className="dropzone-card">
        {busy ? (
          <>
            <div className="dropzone-spinner" />
            <div className="dropzone-title">Adding…</div>
            <div className="dropzone-sub">Hashing and identifying</div>
          </>
        ) : (
          <>
            <svg viewBox="0 0 24 24" width="46" height="46" fill="none"
                 stroke="currentColor" strokeWidth={1.4}
                 strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 16V4" />
              <path d="M7.5 8.5 12 4l4.5 4.5" />
              <path d="M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />
            </svg>
            <div className="dropzone-title">Drop to add</div>
            <div className="dropzone-sub">
              ROMs are added where they are. A folder becomes a library folder.
            </div>
          </>
        )}
      </div>
    </div>
  );
}
