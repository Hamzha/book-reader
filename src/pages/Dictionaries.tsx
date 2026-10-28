import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import type { CatalogEntry, DictProgress, InstalledDict } from "../types";

export default function Dictionaries() {
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [installed, setInstalled] = useState<InstalledDict[]>([]);
  const [progress, setProgress] = useState<DictProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  async function reload() {
    const [nextCatalog, nextInstalled] = await Promise.all([api.dictCatalog(), api.dictInstalled()]);
    setCatalog(nextCatalog);
    setInstalled(nextInstalled);
  }

  useEffect(() => {
    reload().catch((err: unknown) => setError(String(err)));
    let unlisten = () => {};
    listen<DictProgress>("dict-progress", (event) => setProgress(event.payload)).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten();
  }, []);

  async function download(id: string) {
    setBusy(id);
    setError(null);
    try {
      await api.dictDownload(id);
      await reload();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
      setProgress(null);
    }
  }

  return (
    <section className="main">
      <h2>Dictionaries</h2>
      <p className="meta">Download a dictionary once. After that, click a word in a book and the meaning opens offline. Language packs other than the shorter English list are Wiktionary, and their definitions are in English.</p>
      {error && <div className="banner error">{error}</div>}
      <div className="dict-list">
        {catalog.map((entry) => {
          const have = installed.find((item) => item.id === entry.id);
          const current = progress?.id === entry.id ? progress : null;
          const ratio = current && current.total > 0 ? Math.min(100, (current.received / current.total) * 100) : current ? 100 : 0;
          return (
            <article className="dict-card" key={entry.id}>
              <div>
                <h3>{entry.name}</h3>
                <p>
                  {current?.phase === "index" && current.received > 0
                    ? `Indexing ${current.received.toLocaleString()} words`
                    : have
                      ? `${have.words.toLocaleString()} words installed`
                      : entry.sizeHint}
                </p>
                {current && (
                  <div className="meter" aria-label={current.phase}>
                    <i style={{ width: `${current.phase === "index" ? 100 : ratio}%` }} />
                  </div>
                )}
              </div>
              <div>
                {have ? (
                  <button className="ghost" onClick={() => api.dictRemove(entry.id).then(reload)}>Remove</button>
                ) : (
                  <button className="solid" disabled={busy === entry.id} onClick={() => download(entry.id)}>
                    {busy === entry.id ? (current?.phase === "index" ? "Indexing…" : "Downloading…") : "Download"}
                  </button>
                )}
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
