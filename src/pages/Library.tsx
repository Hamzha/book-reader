import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, coverUrl } from "../api";
import { BOOK_EXTENSIONS, type Book, type Shelf, type Stats } from "../types";

type Props = {
  onOpen: (id: string) => void;
  refreshKey: number;
};

const HUES = ["#8d3f24", "#3f5c4b", "#3d4f73", "#6b4b2a", "#5c3d55"];

export default function Library({ onOpen, refreshKey }: Props) {
  const [books, setBooks] = useState<Book[]>([]);
  const [shelves, setShelves] = useState<Shelf[]>([]);
  const [shelfId, setShelfId] = useState<string | null>(null);
  const [sort, setSort] = useState("recent");
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<"grid" | "list">("grid");
  const [stats, setStats] = useState<Stats | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [hot, setHot] = useState(false);
  const [menu, setMenu] = useState<{ book: Book; x: number; y: number } | null>(null);
  const [shelfName, setShelfName] = useState("");

  async function reload() {
    const [nextBooks, nextShelves, nextStats] = await Promise.all([
      api.library(sort, query, shelfId),
      api.shelves(),
      api.stats(),
    ]);
    setBooks(nextBooks);
    setShelves(nextShelves);
    setStats(nextStats);
  }

  useEffect(() => {
    reload().catch((error: unknown) => setMessage(String(error)));
  }, [sort, query, shelfId, refreshKey]);

  useEffect(() => {
    let unlisten = () => {};
    getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over" || event.payload.type === "enter") setHot(true);
      if (event.payload.type === "leave" || event.payload.type === "drop") setHot(false);
      if (event.payload.type === "drop") {
        void ingest(event.payload.paths);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten();
  }, []);

  async function ingest(paths: string[]) {
    setBusy(true);
    setMessage(null);
    try {
      const report = await api.importPaths(paths);
      if (report.failed.length) {
        setMessage(report.failed.map((item) => `${item.path}: ${item.message}`).join("\n"));
      }
      await reload();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function addFiles() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Books", extensions: BOOK_EXTENSIONS }],
    });
    if (Array.isArray(picked)) await ingest(picked);
    else if (typeof picked === "string") await ingest([picked]);
  }

  async function addFolder() {
    const picked = await open({ directory: true });
    if (typeof picked === "string") await ingest([picked]);
  }

  const maxPages = Math.max(1, ...(stats?.days.map((day) => day.pages) ?? [1]));
  const continueBook = useMemo(
    () => books.find((book) => book.lastOpenedAt) ?? books[0],
    [books],
  );

  return (
    <div className="library">
      <aside className="rail">
        <button className={!shelfId ? "active" : ""} onClick={() => setShelfId(null)}>
          <span>All books</span>
          <span className="count">{shelfId ? "" : books.length}</span>
        </button>
        {shelves.map((shelf) => (
          <div className="shelf-row" key={shelf.id}>
            <button className={`grow ${shelfId === shelf.id ? "active" : ""}`} onClick={() => setShelfId(shelf.id)}>
              <span>{shelf.name}</span>
              <span className="count">{shelf.count}</span>
            </button>
            <button className="texty" onClick={() => api.deleteShelf(shelf.id).then(reload)} aria-label={`Delete ${shelf.name}`}>
              ×
            </button>
          </div>
        ))}
        <form
          className="rail-add"
          onSubmit={(event) => {
            event.preventDefault();
            if (!shelfName.trim()) return;
            api.createShelf(shelfName).then(() => {
              setShelfName("");
              reload();
            }).catch((error: unknown) => setMessage(String(error)));
          }}
        >
          <input value={shelfName} onChange={(event) => setShelfName(event.target.value)} placeholder="New shelf" />
          <button className="ghost" type="submit">Add</button>
        </form>
      </aside>
      <section className="main">
        <div className="toolbar">
          <input className="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search title or author" />
          <select value={sort} onChange={(event) => setSort(event.target.value)}>
            <option value="recent">Recent</option>
            <option value="title">Title</option>
            <option value="author">Author</option>
            <option value="progress">Progress</option>
          </select>
          <button className="ghost" onClick={() => setMode(mode === "grid" ? "list" : "grid")}>
            {mode === "grid" ? "List" : "Grid"}
          </button>
          <div className="top-spacer" />
          <button className="ghost" onClick={addFolder}>Add folder</button>
          <button className="solid" onClick={addFiles} disabled={busy}>{busy ? "Importing…" : "Add books"}</button>
        </div>
        {stats && (
          <div className="stats">
            <div>Today {Math.round(stats.todaySeconds / 60)} min · {stats.todayPages} pages</div>
            <div className="bars" title="Pages over recent days">
              {[...stats.days].reverse().map((day) => (
                <span key={day.day} style={{ height: `${Math.max(8, (day.pages / maxPages) * 42)}px` }} title={`${day.day}: ${day.pages} pages`} />
              ))}
            </div>
          </div>
        )}
        {message && <div className="banner error">{message}</div>}
        {continueBook && !query && !shelfId && (
          <div className="continue">
            <div>
              <h2>Continue {continueBook.title}</h2>
              <p>{continueBook.author} · {Math.round(continueBook.progress * 100)}%</p>
            </div>
            <button className="solid" onClick={() => onOpen(continueBook.id)}>Open</button>
          </div>
        )}
        {books.length === 0 ? (
          <div className={`empty ${hot ? "hot" : ""}`}>
            <p>Drop books here, or add a file or a folder.</p>
            <p className="drop-hint">EPUB, PDF, MOBI, AZW, FB2, text, Word, DJVU, and comics.</p>
          </div>
        ) : mode === "grid" ? (
          <div className="grid">
            {books.map((book) => (
              <div className="card-wrap" key={book.id}>
                <button className="book-card" onClick={() => onOpen(book.id)} onContextMenu={(event) => { event.preventDefault(); setMenu({ book, x: event.clientX, y: event.clientY }); }}>
                  <Cover book={book} />
                  <h3>{book.title}</h3>
                  <p>{book.author}</p>
                  <div className="progress"><i style={{ width: `${Math.round(book.progress * 100)}%` }} /></div>
                </button>
                <button className="icon-btn" onClick={(event) => setMenu({ book, x: event.clientX, y: event.clientY })} aria-label="Book actions">⋯</button>
              </div>
            ))}
          </div>
        ) : (
          <div className="list">
            {books.map((book) => (
              <button className="row" key={book.id} onClick={() => onOpen(book.id)} onContextMenu={(event) => { event.preventDefault(); setMenu({ book, x: event.clientX, y: event.clientY }); }}>
                <Cover book={book} />
                <div>
                  <h3>{book.title}</h3>
                  <p>{book.author} · {book.format.toUpperCase()}</p>
                </div>
                <span className="meta">{Math.round(book.progress * 100)}%</span>
              </button>
            ))}
          </div>
        )}
        {menu && (
          <div className="menu" style={{ left: menu.x, top: menu.y }}>
            {shelves.map((shelf) => (
              <button key={shelf.id} onClick={() => { api.addToShelf(shelf.id, menu.book.id).then(() => { setMenu(null); reload(); }); }}>
                Add to {shelf.name}
              </button>
            ))}
            {shelfId && (
              <button onClick={() => api.removeFromShelf(shelfId, menu.book.id).then(() => { setMenu(null); reload(); })}>
                Remove from shelf
              </button>
            )}
            <button onClick={() => api.removeBook(menu.book.id).then(() => { setMenu(null); reload(); })}>
              Remove from library
            </button>
          </div>
        )}
      </section>
    </div>
  );
}

function Cover({ book }: { book: Book }) {
  const [failed, setFailed] = useState(false);
  if (!book.coverPath || failed) {
    const hue = HUES[book.title.length % HUES.length];
    return <div className="cover-ph" style={{ background: hue }}>{book.title.slice(0, 18)}</div>;
  }
  return <img className="cover" src={coverUrl(book.id)} alt="" onError={() => setFailed(true)} />;
}
