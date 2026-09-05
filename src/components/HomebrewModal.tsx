import { useEffect, useState } from "react";

import * as api from "../api";
import { formatSize } from "../api";
import type { HomebrewDetail, HomebrewFile, HomebrewItem } from "../types";

interface Props {
  onClose: () => void;
  onInstalled: () => void;
}

/** Turn a licence URL into something readable. */
function licenceLabel(url: string | null): string | null {
  if (!url) return null;
  const match = url.match(/licenses\/([a-z-]+)\//i);
  if (match) return `CC ${match[1].toUpperCase()}`;
  if (url.includes("publicdomain")) return "Public domain";
  return "Licensed";
}

export default function HomebrewModal({ onClose, onInstalled }: Props) {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<HomebrewItem[]>([]);
  const [collections, setCollections] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [openId, setOpenId] = useState<string | null>(null);
  const [detail, setDetail] = useState<HomebrewDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installed, setInstalled] = useState<Record<string, string>>({});

  const runSearch = async (q: string) => {
    setLoading(true);
    setError(null);
    setOpenId(null);
    setDetail(null);
    try {
      setItems(await api.searchHomebrew(q, 1));
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void api.homebrewCollections().then(setCollections);
    void runSearch("");
    // Only on open — later searches are driven by the form.
  }, []);

  const openItem = async (item: HomebrewItem) => {
    if (openId === item.identifier) {
      setOpenId(null);
      setDetail(null);
      return;
    }
    setOpenId(item.identifier);
    setDetail(null);
    setDetailLoading(true);
    setError(null);
    try {
      setDetail(await api.homebrewFiles(item.identifier));
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setDetailLoading(false);
    }
  };

  const install = async (item: HomebrewItem, file: HomebrewFile) => {
    const key = `${item.identifier}/${file.name}`;
    setInstalling(key);
    setError(null);
    try {
      await api.installHomebrew(item, file, detail?.imageUrl ?? null);
      setInstalled((prev) => ({ ...prev, [key]: item.title }));
      onInstalled();
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setInstalling(null);
    }
  };

  return (
    <div
      className="overlay"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="modal" style={{ maxWidth: 780, height: "84vh" }}>
        <div className="modal-head">
          Homebrew
          <span className="spacer" />
          <button className="btn small ghost" onClick={onClose}>
            ✕
          </button>
        </div>

        <div style={{ padding: "14px 20px 0" }}>
          <form
            className="row"
            onSubmit={(e) => {
              e.preventDefault();
              void runSearch(query);
            }}
          >
            <input
              value={query}
              placeholder="Search homebrew games…"
              onChange={(e) => setQuery(e.target.value)}
            />
            <button className="btn primary" type="submit" disabled={loading}>
              {loading ? "Searching…" : "Search"}
            </button>
          </form>

          <div className="hint" style={{ margin: "10px 0 4px" }}>
            Games written by hobbyists and published freely by their authors,
            from {collections.length} curated Internet Archive collections
            {collections.length > 0 && ` — ${collections.join(", ")}`}. This
            deliberately does not reach the Archive's emulated commercial
            libraries.
          </div>
        </div>

        <div className="modal-body" style={{ paddingTop: 10 }}>
          {error && <div className="error-banner">{error}</div>}

          {!loading && items.length === 0 && (
            <div className="hint">Nothing matched that search.</div>
          )}

          {items.map((item) => {
            const isOpen = openId === item.identifier;
            const licence = licenceLabel(item.license);
            return (
              <div className="folder-row" key={item.identifier} style={{ display: "block" }}>
                <button
                  className="row"
                  style={{ width: "100%", textAlign: "left" }}
                  onClick={() => void openItem(item)}
                >
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ color: "var(--text)", fontWeight: 520 }}>
                      {item.title}
                    </div>
                    <div className="card-sub">
                      {[
                        item.creator,
                        item.year,
                        item.platform,
                        licence,
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </div>
                  </div>
                  <span className="card-sub">{isOpen ? "▾" : "▸"}</span>
                </button>

                {isOpen && (
                  <div style={{ marginTop: 10 }}>
                    {item.description && (
                      <div
                        className="description"
                        style={{ marginBottom: 10, maxHeight: 120, overflow: "auto" }}
                      >
                        {item.description.replace(/<[^>]*>/g, "")}
                      </div>
                    )}

                    {detailLoading && <div className="hint">Loading files…</div>}

                    {detail && detail.files.length === 0 && (
                      <div className="hint">
                        No recognisable ROM in this item.
                      </div>
                    )}

                    {detail?.files.map((file) => {
                      const key = `${item.identifier}/${file.name}`;
                      const done = installed[key];
                      return (
                        <div className="row" key={file.name} style={{ marginTop: 6 }}>
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <div className="card-sub" style={{ color: "var(--text-dim)" }}>
                              {file.name}
                            </div>
                            <div className="card-sub">{formatSize(file.size)}</div>
                          </div>
                          <button
                            className="btn small"
                            disabled={installing === key || !!done}
                            onClick={() => void install(item, file)}
                          >
                            {done
                              ? "In library"
                              : installing === key
                                ? "Downloading…"
                                : "Add to library"}
                          </button>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        <div className="modal-foot">
          <span className="hint" style={{ marginRight: "auto" }}>
            Downloads land in the app data folder and are scanned in
            automatically.
          </span>
          <button className="btn" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
