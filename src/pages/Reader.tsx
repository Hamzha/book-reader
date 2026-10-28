import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { save } from "@tauri-apps/plugin-dialog";
import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { api, comicUrl, djvuUrl, pdfUrl } from "../api";
import { HIGHLIGHTS, STAMPS, type Annotation, type Appearance, type Locator, type OpenedBook, type SearchHit } from "../types";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

const STAMP_MARK: Record<string, string> = {
  star: "★",
  heart: "♥",
  flag: "⚑",
  important: "!",
  reread: "↻",
  question: "?",
};

type Props = {
  bookId: string;
  onBack: () => void;
  onDictionaries: () => void;
};

export default function Reader({ bookId, onBack, onDictionaries }: Props) {
  const [opened, setOpened] = useState<OpenedBook | null>(null);
  const [appearance, setAppearance] = useState<Appearance | null>(null);
  const [chapter, setChapter] = useState(0);
  const [page, setPage] = useState(0);
  const [pagesInView, setPagesInView] = useState(1);
  const [html, setHtml] = useState("");
  const [marks, setMarks] = useState<Annotation[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [panel, setPanel] = useState<"toc" | "notes" | "type" | null>(null);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [dict, setDict] = useState<{ word: string; text: string; ipa?: string | null; x: number; y: number } | null>(null);
  const [selection, setSelection] = useState<{ text: string; x: number; y: number } | null>(null);
  const [noteDraft, setNoteDraft] = useState("");
  const [zoom, setZoom] = useState<"fit-width" | "fit-page" | "custom">("fit-page");
  const [scale, setScale] = useState(1);
  const [pace, setPace] = useState(45);
  const frameRef = useRef<HTMLIFrameElement>(null);
  const lastPage = useRef(0);
  const pageRef = useRef(0);
  pageRef.current = page;

  useEffect(() => {
    let stop = false;
    Promise.all([api.openBook(bookId), api.getSettings(), api.annotations(bookId), api.stats()])
      .then(([book, settings, notes, stats]) => {
        if (stop) return;
        setOpened(book);
        setAppearance(settings);
        setMarks(notes);
        setPace(stats.secondsPerPage || 45);
        const start = book.book.position;
        setChapter(start?.chapter ?? 0);
        setPage(start?.page ?? 0);
        lastPage.current = start?.page ?? 0;
      })
      .catch((err: unknown) => setError(String(err)));
    return () => {
      stop = true;
    };
  }, [bookId]);

  useEffect(() => {
    if (!opened || opened.kind !== "reflow") return;
    api.chapterHtml(bookId, chapter).then(setHtml).catch((err: unknown) => setError(String(err)));
  }, [opened, bookId, chapter]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      const moved = Math.max(0, page - lastPage.current);
      lastPage.current = page;
      api.tick(bookId, 15, moved).catch(() => undefined);
    }, 15000);
    return () => window.clearInterval(timer);
  }, [bookId, page]);

  function persist(nextChapter: number, nextPage: number, inChapter: number) {
    if (!opened) return;
    const total = Math.max(1, opened.chapterCount);
    const progress = opened.kind === "reflow"
      ? (nextChapter + (inChapter > 0 ? nextPage / inChapter : 0)) / total
      : (nextPage + 1) / total;
    api.savePosition(bookId, { chapter: nextChapter, page: nextPage, pagesInChapter: inChapter }, progress).catch(() => undefined);
  }

  function turn(delta: number) {
    if (!opened) return;
    if (opened.kind === "reflow") {
      const frame = frameRef.current;
      const doc = frame?.contentDocument;
      const width = frame?.clientWidth ?? 1;
      if (!doc) return;
      const max = Math.max(0, Math.ceil(doc.documentElement.scrollWidth / width) - 1);
      const next = page + delta;
      if (next < 0 && chapter > 0) {
        setChapter(chapter - 1);
        setPage(9999);
        return;
      }
      if (next > max && chapter < opened.chapterCount - 1) {
        setChapter(chapter + 1);
        setPage(0);
        persist(chapter + 1, 0, 1);
        return;
      }
      const clamped = Math.max(0, Math.min(max, next));
      setPage(clamped);
      doc.documentElement.scrollLeft = clamped * width;
      setPagesInView(max + 1);
      persist(chapter, clamped, max + 1);
      return;
    }
    const total = opened.kind === "comic" ? opened.imageCount : opened.chapterCount;
    const next = Math.max(0, Math.min(total - 1, page + delta));
    setPage(next);
    persist(next, next, total);
  }

  function bumpZoom(direction: 1 | -1) {
    if (opened?.kind === "reflow") {
      if (!appearance) return;
      const fontSize = Math.min(48, Math.max(12, appearance.fontSize + direction * 2));
      const next = { ...appearance, fontSize };
      setAppearance(next);
      api.saveSettings(next).catch(() => undefined);
      return;
    }
    setScale((current) => {
      const stepped = direction > 0 ? current * 1.25 : current / 1.25;
      return Math.min(4, Math.max(0.5, Math.round(stepped * 100) / 100));
    });
  }

  function resetZoom() {
    if (opened?.kind === "reflow") {
      if (!appearance) return;
      const next = { ...appearance, fontSize: 20 };
      setAppearance(next);
      api.saveSettings(next).catch(() => undefined);
      return;
    }
    setZoom("fit-page");
    setScale(1);
  }

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
      if (event.ctrlKey && (event.key === "=" || event.key === "+" || event.key === "-" || event.key === "_" || event.key === "0")) {
        event.preventDefault();
        if (event.key === "0") resetZoom();
        else bumpZoom(event.key === "-" || event.key === "_" ? -1 : 1);
        return;
      }
      if (event.key === "ArrowRight" || event.key === "PageDown" || event.key === " ") {
        event.preventDefault();
        turn(1);
      }
      if (event.key === "ArrowLeft" || event.key === "PageUp") {
        event.preventDefault();
        turn(-1);
      }
      if (event.key === "f") {
        getCurrentWindow().isFullscreen().then((on) => getCurrentWindow().setFullscreen(!on));
      }
      if (event.key === "Escape") setPanel(null);
      if (event.key === "b") void addMark("bookmark", {});
    }
    function onWheel(event: WheelEvent) {
      if (!event.ctrlKey) return;
      event.preventDefault();
      bumpZoom(event.deltaY < 0 ? 1 : -1);
    }
    window.addEventListener("keydown", onKey);
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("wheel", onWheel);
    };
  });

  async function addMark(kind: string, extra: { color?: string; stamp?: string; text?: string; note?: string }) {
    if (!opened) return;
    const locator: Locator = {
      chapter: opened.kind === "reflow" ? chapter : page,
      page,
      quote: extra.text ?? selection?.text ?? null,
    };
    const mark = await api.addAnnotation({
      bookId,
      kind,
      locator,
      color: extra.color,
      stamp: extra.stamp,
      text: extra.text ?? selection?.text,
      note: extra.note,
    });
    setMarks((current) => [mark, ...current]);
    setSelection(null);
    setNoteDraft("");
    paintMarks([mark, ...marks]);
  }

  function paintMarks(list: Annotation[]) {
    const doc = frameRef.current?.contentDocument;
    if (!doc?.body) return;
    doc.querySelectorAll("mark.hl").forEach((node) => {
      const parent = node.parentNode;
      if (!parent) return;
      parent.replaceChild(doc.createTextNode(node.textContent ?? ""), node);
      parent.normalize();
    });
    for (const mark of list) {
      if (mark.kind !== "highlight" && mark.kind !== "note") continue;
      if (mark.locator.chapter !== chapter) continue;
      const quote = mark.text ?? mark.locator.quote;
      if (!quote) continue;
      wrapQuote(doc.body, quote, mark.color || "#f6e27a");
    }
  }

  useEffect(() => {
    if (!appearance || opened?.kind !== "reflow" || !html) return;
    const frame = frameRef.current;
    const doc = frame?.contentDocument;
    if (!doc) return;
    const theme = themeColors(appearance);
    const width = Math.max(1, frame.clientWidth);
    const height = Math.max(1, frame.clientHeight);
    const pad = appearance.margin * 2;
    const gap = appearance.spread ? 40 : 0;
    const contentWidth = Math.max(160, width - pad);
    const column = appearance.spread ? Math.max(120, (contentWidth - gap) / 2) : contentWidth;
    doc.open();
    doc.write(pageDocument(html, appearance, theme, { width, height, column, gap }));
    doc.close();
    doc.addEventListener("wheel", (event) => {
      if (!event.ctrlKey) return;
      event.preventDefault();
      bumpZoom(event.deltaY < 0 ? 1 : -1);
    }, { passive: false });
    doc.addEventListener("keydown", (event) => {
      if (!event.ctrlKey) return;
      if (event.key !== "=" && event.key !== "+" && event.key !== "-" && event.key !== "_" && event.key !== "0") return;
      event.preventDefault();
      if (event.key === "0") resetZoom();
      else bumpZoom(event.key === "-" || event.key === "_" ? -1 : 1);
    });
    const apply = () => {
      paintMarks(marks);
      const viewWidth = Math.max(1, frame.clientWidth);
      const max = Math.max(0, Math.round(doc.documentElement.scrollWidth / viewWidth) - 1);
      const start = Math.min(pageRef.current, max);
      doc.documentElement.scrollLeft = start * viewWidth;
      setPagesInView(max + 1);
      if (pageRef.current > max) setPage(start);
    };
    requestAnimationFrame(apply);
  }, [html, appearance, chapter, opened?.kind]);

  function onFrameClick(event: React.MouseEvent) {
    const doc = frameRef.current?.contentDocument;
    const frame = frameRef.current;
    if (!doc || !frame) return;
    const selected = doc.getSelection()?.toString().trim() ?? "";
    const rect = frame.getBoundingClientRect();
    if (selected.length > 1) {
      setSelection({ text: selected, x: event.clientX, y: event.clientY });
      setDict(null);
      return;
    }
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;
    const word = wordAt(doc, x, y);
    if (!word) return;
    lookup(word, event.clientX, event.clientY);
  }

  async function lookup(word: string, x: number, y: number) {
    const found = await api.dictLookup(word).catch(() => null);
    let text = found?.definition;
    if (!text) {
      const installed = await api.dictInstalled().catch(() => []);
      text = installed.length === 0
        ? "No dictionary is installed yet. Open Dictionaries and download one, then click the word again."
        : `No entry for “${word}”.`;
    }
    setSelection(null);
    setDict({
      word: found?.word || word,
      text,
      ipa: found?.ipa,
      x,
      y,
    });
  }

  async function runSearch(value: string) {
    setQuery(value);
    if (value.trim().length < 2 || !opened) {
      setHits([]);
      return;
    }
    if (opened.kind === "document") {
      setHits(await searchPdf(pdfUrl(bookId), value.trim()));
      return;
    }
    setHits(await api.searchBook(bookId, value.trim()));
  }

  const total = opened ? (opened.kind === "comic" ? opened.imageCount : opened.chapterCount) : 1;
  const here = opened?.kind === "reflow" ? chapter : page;
  const fraction = opened?.kind === "reflow"
    ? (chapter + (pagesInView > 0 ? page / pagesInView : 0)) / Math.max(1, total)
    : (page + 1) / Math.max(1, total);
  const remaining = Math.max(0, total - here - 1);
  const minutes = Math.max(1, Math.round((remaining * pace) / 60));
  const theme = appearance ? themeColors(appearance) : { bg: "#f6f0e6", fg: "#241c14" };

  const pageMarks = useMemo(
    () => marks.filter((mark) => mark.kind === "stamp" && (mark.locator.chapter === here || mark.locator.page === page)),
    [marks, here, page],
  );

  if (!opened || !appearance) {
    return <div className="reader"><p className="status-line">{error ?? "Opening…"}</p></div>;
  }

  return (
    <div className="reader">
      <div className="reader-bar">
        <button className="ghost" onClick={onBack}>Library</button>
        <span className="title">{opened.book.title}</span>
        <button className="ghost" onClick={() => setPanel(panel === "toc" ? null : "toc")}>Contents</button>
        <button className="ghost" onClick={() => setPanel(panel === "notes" ? null : "notes")}>Notebook</button>
        <button className="ghost" onClick={() => setPanel(panel === "type" ? null : "type")}>Type</button>
        <input className="search" value={query} placeholder="Search" onChange={(event) => runSearch(event.target.value)} />
        <button className="ghost" onClick={() => turn(-1)}>Prev</button>
        <button className="ghost" onClick={() => turn(1)}>Next</button>
        <button className="ghost" onClick={() => addMark("bookmark", {})}>Bookmark</button>
        <button className="ghost" onClick={() => { void speakCurrent(opened, html, page); }}>Speak</button>
        <div className="zoom-controls">
          <button className="ghost" title="Zoom out" onClick={() => bumpZoom(-1)}>−</button>
          <button className="ghost zoom-label" title="Fit the page" onClick={resetZoom}>
            {opened.kind === "reflow" ? appearance.fontSize : zoom === "fit-width" && scale === 1 ? "Width" : `${Math.round(scale * 100)}%`}
          </button>
          <button className="ghost" title="Zoom in" onClick={() => bumpZoom(1)}>+</button>
        </div>
        <button className="ghost" onClick={() => getCurrentWindow().isFullscreen().then((on) => getCurrentWindow().setFullscreen(!on))}>Full</button>
      </div>
      <div className="reader-stage">
        {panel === "toc" && (
          <aside className="side">
            <h3>Contents</h3>
            {opened.toc.map((item, index) => (
              <button className={`item ${item.chapter === chapter ? "active" : ""}`} key={`${item.title}-${index}`} onClick={() => { setChapter(item.chapter); setPage(item.chapter); }}>
                {item.title}
              </button>
            ))}
            {hits.length > 0 && <h3>Search</h3>}
            {hits.map((hit, index) => (
              <button className="hit" key={`${hit.chapter}-${index}`} onClick={() => { setChapter(hit.chapter); setPage(hit.chapter); }}>
                {hit.snippet}
              </button>
            ))}
          </aside>
        )}
        <div className="page-wrap">
          <div className="sheet" style={{ background: theme.bg, color: theme.fg, ["--page" as string]: theme.bg }}>
            {opened.kind === "reflow" && (
              <iframe
                ref={frameRef}
                title={opened.book.title}
                scrolling={appearance.viewMode === "scroll" ? "yes" : "no"}
                onMouseUp={onFrameClick}
                onWheel={(event) => {
                  if (event.ctrlKey) {
                    event.preventDefault();
                    bumpZoom(event.deltaY < 0 ? 1 : -1);
                    return;
                  }
                  if (appearance.viewMode === "scroll") return;
                  event.preventDefault();
                  if (Math.abs(event.deltaY) + Math.abs(event.deltaX) < 8) return;
                  turn(event.deltaY + event.deltaX > 0 ? 1 : -1);
                }}
              />
            )}
            {opened.kind === "document" && (
              <PdfPage url={pdfUrl(bookId)} page={page + 1} zoom={zoom} scale={scale} onWord={(word, x, y) => lookup(word, x, y)} />
            )}
            {opened.kind === "djvu" && <ZoomedPicture src={djvuUrl(bookId, page)} zoom={zoom} scale={scale} />}
            {opened.kind === "comic" && <ZoomedPicture src={comicUrl(bookId, page)} zoom={zoom} scale={scale} />}
            <div className="margin-stamps">
              {pageMarks.map((mark) => (
                <button key={mark.id} title={mark.stamp ?? "stamp"}>{STAMP_MARK[mark.stamp ?? "star"]}</button>
              ))}
            </div>
          </div>
        </div>
        {panel === "notes" && (
          <Notebook
            marks={marks}
            onJump={(mark) => {
              setChapter(mark.locator.chapter);
              setPage(mark.locator.page);
            }}
            onDelete={async (id) => {
              await api.deleteAnnotation(id);
              setMarks(await api.annotations(bookId));
            }}
            onExport={async (format) => {
              const path = await save({ defaultPath: `${opened.book.title}-notes.${format === "md" ? "md" : "txt"}` });
              if (typeof path === "string") await api.exportAnnotations(bookId, format, path);
            }}
          />
        )}
        {panel === "type" && (
          <AppearancePanel
            appearance={appearance}
            zoom={zoom}
            onZoom={(mode) => { setZoom(mode); setScale(1); }}
            onStep={bumpZoom}
            onReset={resetZoom}
            onChange={(next) => {
              setAppearance(next);
              api.saveSettings(next).catch(() => undefined);
            }}
          />
        )}
      </div>
      <div className="status-line">
        <span>{Math.round(fraction * 100)}% · {opened.kind === "reflow" ? `Chapter ${chapter + 1} of ${total}` : `Page ${page + 1} of ${total}`}</span>
        <span>About {minutes} min left</span>
        {error && <span className="warn">{error}</span>}
      </div>
      {selection && (
        <div className="popover" style={{ left: selection.x, top: selection.y + 12 }}>
          <div className="swatches">
            {HIGHLIGHTS.map((color) => (
              <button key={color} style={{ background: color }} onClick={() => addMark("highlight", { color, text: selection.text })} />
            ))}
          </div>
          <div className="stamp-row">
            {STAMPS.map((stamp) => (
              <button key={stamp} className="ghost" onClick={() => addMark("stamp", { stamp, text: selection.text })}>{STAMP_MARK[stamp]}</button>
            ))}
          </div>
          <textarea className="note-box" value={noteDraft} placeholder="Add a note" onChange={(event) => setNoteDraft(event.target.value)} />
          <button className="solid" onClick={() => addMark("note", { text: selection.text, note: noteDraft })}>Save note</button>
        </div>
      )}
      {dict && (
        <div className="dict-pop" style={{ left: dict.x, top: dict.y + 12 }}>
          <h4>{dict.word}</h4>
          {dict.ipa && <div className="ipa">{dict.ipa}</div>}
          <p>{dict.text}</p>
          <button className="ghost" onClick={() => addMark("highlight", { color: HIGHLIGHTS[0], text: dict.word })}>Highlight</button>
          <button className="texty" onClick={() => { setSelection({ text: dict.word, x: dict.x, y: dict.y }); setDict(null); }}>Add note</button>
          <button className="texty" onClick={onDictionaries}>Dictionaries</button>
          <button className="texty" onClick={() => setDict(null)}>Close</button>
        </div>
      )}
    </div>
  );
}

function Notebook({ marks, onJump, onDelete, onExport }: {
  marks: Annotation[];
  onJump: (mark: Annotation) => void;
  onDelete: (id: string) => void;
  onExport: (format: string) => void;
}) {
  return (
    <aside className="side right">
      <h3>Notebook</h3>
      <button className="ghost" onClick={() => onExport("md")}>Export Markdown</button>
      <button className="texty" onClick={() => onExport("txt")}>Export text</button>
      {marks.length === 0 && <p className="meta">Highlights, notes, bookmarks, and stamps land here.</p>}
      {marks.map((mark) => (
        <div className="note-card" key={mark.id}>
          <button className="item" onClick={() => onJump(mark)}>
            <strong>{mark.kind === "stamp" ? `${STAMP_MARK[mark.stamp ?? "star"]} ${mark.stamp}` : mark.kind}</strong>
            <div>{mark.text || mark.note || "Saved place"}</div>
            {mark.note && mark.kind === "note" && <div>{mark.note}</div>}
          </button>
          <button className="texty" onClick={() => onDelete(mark.id)}>Delete</button>
        </div>
      ))}
    </aside>
  );
}

function AppearancePanel({ appearance, onChange, zoom, onZoom, onStep, onReset }: {
  appearance: Appearance;
  onChange: (next: Appearance) => void;
  zoom: "fit-width" | "fit-page" | "custom";
  onZoom: (zoom: "fit-width" | "fit-page" | "custom") => void;
  onStep: (direction: 1 | -1) => void;
  onReset: () => void;
}) {
  function patch(partial: Partial<Appearance>) {
    onChange({ ...appearance, ...partial });
  }
  return (
    <aside className="side right">
      <h3>Type</h3>
      <label className="field">Font
        <select value={appearance.fontFamily} onChange={(event) => patch({ fontFamily: event.target.value })}>
          {["Georgia", "Palatino Linotype", "Cambria", "Constantia", "Segoe UI"].map((font) => <option key={font}>{font}</option>)}
        </select>
      </label>
      <label className="field">Size
        <input type="range" min={12} max={48} value={appearance.fontSize} onChange={(event) => patch({ fontSize: Number(event.target.value) })} />
      </label>
      <label className="field">Line height
        <input type="range" min={1.2} max={2.1} step={0.05} value={appearance.lineHeight} onChange={(event) => patch({ lineHeight: Number(event.target.value) })} />
      </label>
      <label className="field">Margins
        <input type="range" min={16} max={96} value={appearance.margin} onChange={(event) => patch({ margin: Number(event.target.value) })} />
      </label>
      <label className="field">Alignment
        <select value={appearance.align} onChange={(event) => patch({ align: event.target.value })}>
          <option value="left">Left</option>
          <option value="justify">Justify</option>
        </select>
      </label>
      <div className="themes">
        {["light", "sepia", "dark", "custom"].map((theme) => (
          <button key={theme} className={appearance.theme === theme ? "on" : ""} onClick={() => patch({ theme })}>{theme}</button>
        ))}
      </div>
      <label className="field">View
        <select value={appearance.viewMode} onChange={(event) => patch({ viewMode: event.target.value })}>
          <option value="paginated">Pages</option>
          <option value="scroll">Scroll</option>
        </select>
      </label>
      <label className="field"><input type="checkbox" checked={appearance.spread} onChange={(event) => patch({ spread: event.target.checked })} /> Two-page spread</label>
      <h3>Zoom</h3>
      <div className="themes">
        <button className={zoom === "fit-width" ? "on" : ""} onClick={() => onZoom("fit-width")}>Fit width</button>
        <button className={zoom === "fit-page" ? "on" : ""} onClick={() => onZoom("fit-page")}>Fit page</button>
        <button onClick={() => onStep(-1)}>Zoom out</button>
        <button onClick={onReset}>100%</button>
        <button onClick={() => onStep(1)}>Zoom in</button>
      </div>
      {appearance.theme === "custom" && (
        <>
          <label className="field">Text <input type="color" value={appearance.customFg} onChange={(event) => patch({ customFg: event.target.value })} /></label>
          <label className="field">Paper <input type="color" value={appearance.customBg} onChange={(event) => patch({ customBg: event.target.value })} /></label>
        </>
      )}
    </aside>
  );
}

function themeColors(appearance: Appearance) {
  if (appearance.theme === "dark") return { bg: "#1d1a17", fg: "#f3eadf" };
  if (appearance.theme === "sepia") return { bg: "#f3e6cf", fg: "#3a2a1a" };
  if (appearance.theme === "custom") return { bg: appearance.customBg, fg: appearance.customFg };
  return { bg: "#f7f1e6", fg: "#241c14" };
}

function pageDocument(
  html: string,
  appearance: Appearance,
  theme: { bg: string; fg: string },
  layout: { width: number; height: number; column: number; gap: number },
) {
  const mode = appearance.viewMode === "scroll" ? "scroll" : "paginated";
  return `<!doctype html><html class="${mode}"><head><meta charset="utf-8"><style>
    html, body { margin: 0; background: ${theme.bg}; color: ${theme.fg}; }
    body { font-family: "${appearance.fontFamily}", Georgia, serif; font-size: ${appearance.fontSize}px; line-height: ${appearance.lineHeight}; text-align: ${appearance.align}; }
    html.paginated, body.paginated {
      width: ${Math.round(layout.width)}px;
      height: ${Math.round(layout.height)}px;
      overflow: hidden;
    }
    body.paginated {
      box-sizing: border-box;
      padding: ${appearance.margin}px;
      column-width: ${Math.round(layout.column)}px;
      column-gap: ${Math.round(layout.gap)}px;
      column-fill: auto;
    }
    html.scroll, body.scroll { width: 100%; height: auto; overflow-x: hidden; }
    body.scroll { min-height: 100%; padding: ${appearance.margin}px; max-width: 42rem; margin: 0 auto; box-sizing: border-box; }
    img, svg, video, table, pre { max-width: 100%; height: auto; }
    p { margin: 0 0 0.9em; }
    a { color: inherit; }
    mark.hl { background: var(--hl, #f6e27a); color: inherit; }
  </style></head><body class="${mode}">${html}</body></html>`;
}

function wrapQuote(root: HTMLElement, quote: string, color: string) {
  const walker = root.ownerDocument!.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  let full = "";
  const starts: number[] = [];
  let node = walker.nextNode();
  while (node) {
    starts.push(full.length);
    nodes.push(node as Text);
    full += node.textContent ?? "";
    node = walker.nextNode();
  }
  const index = full.toLowerCase().indexOf(quote.toLowerCase());
  if (index < 0) return;
  const end = index + quote.length;
  for (let i = 0; i < nodes.length; i += 1) {
    const start = starts[i];
    const text = nodes[i].textContent ?? "";
    const stop = start + text.length;
    if (stop <= index || start >= end) continue;
    const localStart = Math.max(0, index - start);
    const localEnd = Math.min(text.length, end - start);
    const range = root.ownerDocument!.createRange();
    range.setStart(nodes[i], localStart);
    range.setEnd(nodes[i], localEnd);
    const mark = root.ownerDocument!.createElement("mark");
    mark.className = "hl";
    mark.style.background = color;
    try {
      range.surroundContents(mark);
    } catch {
      return;
    }
  }
}

function sliceWord(text: string, offset: number) {
  const letter = (ch: string) => /[\p{L}\p{N}'’-]/u.test(ch);
  if (!text) return "";
  let index = Math.max(0, Math.min(offset, text.length));
  if (index >= text.length || !letter(text[index] ?? "")) {
    if (index > 0 && letter(text[index - 1] ?? "")) index -= 1;
  }
  if (!letter(text[index] ?? "")) return "";
  let start = index;
  let end = index + 1;
  while (start > 0 && letter(text[start - 1] ?? "")) start -= 1;
  while (end < text.length && letter(text[end] ?? "")) end += 1;
  return text.slice(start, end);
}

function wordAt(doc: Document, x: number, y: number) {
  const range = doc.caretRangeFromPoint?.(x, y);
  if (!range || range.startContainer.nodeType !== Node.TEXT_NODE) return "";
  return sliceWord(range.startContainer.textContent ?? "", range.startOffset);
}

function fittedScale(frameW: number, frameH: number, pageW: number, pageH: number, zoom: string) {
  if (frameW < 20 || frameH < 20 || pageW < 1 || pageH < 1) return 1;
  if (zoom === "fit-width") return frameW / pageW;
  return Math.min(frameW / pageW, frameH / pageH);
}

function ZoomedPicture({ src, zoom, scale }: { src: string; zoom: string; scale: number }) {
  const host = useRef<HTMLDivElement>(null);
  const [frame, setFrame] = useState({ width: 0, height: 0 });
  const [natural, setNatural] = useState({ width: 0, height: 0 });

  useEffect(() => {
    setNatural({ width: 0, height: 0 });
  }, [src]);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const measure = () => setFrame({ width: el.clientWidth, height: el.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const fitted = fittedScale(frame.width, frame.height, natural.width, natural.height, zoom);
  const width = Math.max(1, Math.round(natural.width * fitted * scale));
  const height = Math.max(1, Math.round(natural.height * fitted * scale));
  const pan = natural.width > 0 && (width > frame.width + 1 || height > frame.height + 1);

  return (
    <div ref={host} className={pan ? "doc-scroll can-pan" : "doc-scroll"}>
      <img
        className="page"
        alt=""
        src={src}
        onLoad={(event) => setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })}
        style={natural.width > 0 ? { width, height, maxWidth: "none", maxHeight: "none" } : undefined}
      />
    </div>
  );
}

async function searchPdf(url: string, query: string): Promise<SearchHit[]> {
  const pdf = await pdfjs.getDocument({ url, disableRange: true, disableStream: true }).promise;
  const hits: SearchHit[] = [];
  const needle = query.toLowerCase();
  for (let number = 1; number <= pdf.numPages && hits.length < 30; number += 1) {
    const page = await pdf.getPage(number);
    const content = await page.getTextContent();
    const text = content.items.map((item) => ("str" in item ? item.str : "")).join(" ");
    const at = text.toLowerCase().indexOf(needle);
    if (at >= 0) {
      hits.push({ chapter: number - 1, snippet: text.slice(Math.max(0, at - 30), at + query.length + 40) });
    }
  }
  return hits;
}

async function speakCurrent(opened: OpenedBook, html: string, page: number) {
  window.speechSynthesis.cancel();
  let source = `Page ${page + 1} of ${opened.book.title}`;
  if (opened.kind === "reflow") {
    source = html.replace(/<[^>]+>/g, " ");
  } else if (opened.kind === "document") {
    const pdf = await pdfjs.getDocument({ url: pdfUrl(opened.book.id), disableRange: true, disableStream: true }).promise;
    const pdfPage = await pdf.getPage(page + 1);
    const content = await pdfPage.getTextContent();
    source = content.items.map((item) => ("str" in item ? item.str : "")).join(" ");
  }
  const utterance = new SpeechSynthesisUtterance(source.replace(/\s+/g, " ").trim().slice(0, 4000));
  window.speechSynthesis.speak(utterance);
}

function PdfPage({ url, page, zoom, scale, onWord }: {
  url: string;
  page: number;
  zoom: "fit-width" | "fit-page" | "custom";
  scale: number;
  onWord: (word: string, x: number, y: number) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const [items, setItems] = useState<{ str: string; left: number; top: number; size: number }[]>([]);
  const [box, setBox] = useState({ width: 0, height: 0 });
  const [frame, setFrame] = useState({ width: 0, height: 0 });

  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;
    const measure = () => setFrame({ width: el.clientWidth, height: el.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (frame.width < 20 || frame.height < 20) return;
    let cancel = false;
    (async () => {
      const pdf = await pdfjs.getDocument({ url, disableRange: true, disableStream: true }).promise;
      const pdfPage = await pdf.getPage(page);
      const base = pdfPage.getViewport({ scale: 1 });
      const next = fittedScale(frame.width, frame.height, base.width, base.height, zoom === "fit-width" ? "fit-width" : "fit-page") * scale;
      const viewport = pdfPage.getViewport({ scale: next });
      const canvas = canvasRef.current;
      if (!canvas || cancel) return;
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      setBox({ width: viewport.width, height: viewport.height });
      const context = canvas.getContext("2d");
      if (!context) return;
      await pdfPage.render({ canvasContext: context, viewport }).promise;
      const text = await pdfPage.getTextContent();
      if (cancel) return;
      setItems(text.items.flatMap((item) => {
        if (!("str" in item) || !item.str) return [];
        const tx = item.transform;
        const size = Math.hypot(tx[2], tx[3]) * next;
        return [{
          str: item.str,
          left: tx[4] * next,
          top: viewport.height - tx[5] * next - size,
          size,
        }];
      }));
    })().catch(() => undefined);
    return () => {
      cancel = true;
    };
  }, [url, page, zoom, scale, frame.width, frame.height]);

  return (
    <div ref={scrollerRef} className={box.width > frame.width + 1 || box.height > frame.height + 1 ? "doc-scroll can-pan" : "doc-scroll"}>
      <div style={{ position: "relative", width: box.width, height: box.height }}>
        <canvas ref={canvasRef} />
        <div className="text-layer" style={{ width: box.width, height: box.height }}>
          {items.map((item, index) => (
            <span
              key={`${item.left}-${index}`}
              style={{ left: item.left, top: item.top, fontSize: item.size }}
              onClick={(event) => {
                const range = document.caretRangeFromPoint?.(event.clientX, event.clientY);
                const fromCaret = range && range.startContainer.nodeType === Node.TEXT_NODE
                  ? sliceWord(range.startContainer.textContent ?? "", range.startOffset)
                  : "";
                const raw = event.currentTarget.textContent?.trim() ?? "";
                const word = fromCaret || (raw && !/\s/u.test(raw) ? raw : "");
                if (word) onWord(word, event.clientX, event.clientY);
              }}
            >
              {item.str}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
