import { invoke } from "@tauri-apps/api/core";
import type {
  Annotation,
  Appearance,
  Book,
  CatalogEntry,
  Definition,
  ImportReport,
  InstalledDict,
  Locator,
  OpenedBook,
  Position,
  SearchHit,
  Shelf,
  Stats,
} from "./types";

export const api = {
  library: (sort: string, query: string, shelfId: string | null) =>
    invoke<Book[]>("library_list", { sort, query, shelfId }),
  shelves: () => invoke<Shelf[]>("shelf_list"),
  createShelf: (name: string) => invoke<Shelf>("shelf_create", { name }),
  deleteShelf: (id: string) => invoke<void>("shelf_delete", { id }),
  addToShelf: (shelfId: string, bookId: string) => invoke<void>("shelf_add_book", { shelfId, bookId }),
  removeFromShelf: (shelfId: string, bookId: string) => invoke<void>("shelf_remove_book", { shelfId, bookId }),
  importPaths: (paths: string[]) => invoke<ImportReport>("import_paths", { paths }),
  removeBook: (id: string) => invoke<void>("remove_book", { id }),
  openBook: (id: string) => invoke<OpenedBook>("open_book", { id }),
  chapterHtml: (bookId: string, index: number) => invoke<string>("chapter_html", { bookId, index }),
  savePosition: (bookId: string, position: Position, progress: number) =>
    invoke<void>("save_position", { bookId, position, progress }),
  searchBook: (bookId: string, query: string) => invoke<SearchHit[]>("search_book", { bookId, query }),
  getSettings: () => invoke<Appearance>("get_settings"),
  saveSettings: (appearance: Appearance) => invoke<void>("save_settings", { appearance }),
  dictCatalog: () => invoke<CatalogEntry[]>("dict_catalog"),
  dictInstalled: () => invoke<InstalledDict[]>("dict_installed"),
  dictDownload: (id: string) => invoke<InstalledDict>("dict_download", { id }),
  dictRemove: (id: string) => invoke<void>("dict_remove", { id }),
  dictLookup: (word: string) => invoke<Definition | null>("dict_lookup", { word }),
  annotations: (bookId: string) => invoke<Annotation[]>("annotations_list", { bookId }),
  addAnnotation: (input: {
    bookId: string;
    kind: string;
    locator: Locator;
    color?: string | null;
    stamp?: string | null;
    text?: string | null;
    note?: string | null;
  }) => invoke<Annotation>("annotation_add", input),
  deleteAnnotation: (id: string) => invoke<void>("annotation_delete", { id }),
  exportAnnotations: (bookId: string, format: string, path: string) =>
    invoke<void>("annotations_export", { bookId, format, path }),
  stats: () => invoke<Stats>("stats_summary"),
  tick: (bookId: string, seconds: number, pages: number) =>
    invoke<void>("tick_reading", { bookId, seconds, pages }),
  takeLaunchFiles: () => invoke<string[]>("take_launch_files"),
  bookByPath: (path: string) => invoke<Book | null>("book_by_path", { path }),
  convertPdfToMobi: (bookId: string, dest: string) => invoke<Book>("convert_pdf_to_mobi", { bookId, dest }),
};

export function coverUrl(id: string) {
  return `http://book.localhost/cover/${id}`;
}

export function pdfUrl(id: string) {
  return `http://book.localhost/pdf/${id}`;
}

export function comicUrl(id: string, index: number) {
  return `http://book.localhost/comic/${id}/${index}`;
}

export function djvuUrl(id: string, index: number) {
  return `http://book.localhost/djvu/${id}/${index}`;
}
