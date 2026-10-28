export type Position = {
  chapter: number;
  page: number;
  pagesInChapter: number;
};

export type Book = {
  id: string;
  path: string;
  title: string;
  author: string;
  format: string;
  kind: string;
  coverPath: string | null;
  addedAt: number;
  lastOpenedAt: number | null;
  progress: number;
  pageCount: number;
  position: Position | null;
};

export type Shelf = {
  id: string;
  name: string;
  count: number;
};

export type TocItem = {
  title: string;
  chapter: number;
};

export type OpenedBook = {
  book: Book;
  kind: string;
  toc: TocItem[];
  chapterCount: number;
  imageCount: number;
};

export type SearchHit = {
  chapter: number;
  snippet: string;
};

export type Locator = {
  chapter: number;
  page: number;
  quote?: string | null;
};

export type Annotation = {
  id: string;
  bookId: string;
  kind: string;
  locator: Locator;
  color?: string | null;
  stamp?: string | null;
  text?: string | null;
  note?: string | null;
  createdAt: number;
};

export type Appearance = {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  margin: number;
  align: string;
  theme: string;
  customFg: string;
  customBg: string;
  viewMode: string;
  spread: boolean;
  windowWidth: number;
  windowHeight: number;
};

export type ImportReport = {
  added: Book[];
  failed: { path: string; message: string }[];
};

export type CatalogEntry = {
  id: string;
  lang: string;
  name: string;
  sizeHint: string;
};

export type InstalledDict = {
  id: string;
  lang: string;
  name: string;
  installedAt: number;
  words: number;
};

export type Definition = {
  word: string;
  definition: string;
  ipa?: string | null;
  dictionary: string;
};

export type DictProgress = {
  id: string;
  received: number;
  total: number;
  phase: string;
};

export type Stats = {
  todaySeconds: number;
  todayPages: number;
  days: { day: string; seconds: number; pages: number }[];
  secondsPerPage: number;
};

export const BOOK_EXTENSIONS = [
  "epub", "pdf", "mobi", "azw", "azw3", "fb2", "txt", "html", "htm",
  "md", "markdown", "rtf", "docx", "djvu", "djv", "cbz", "cbr", "cb7",
];

export const STAMPS = ["star", "heart", "flag", "important", "reread", "question"] as const;
export const HIGHLIGHTS = ["#f6e27a", "#b7e0b2", "#a9d4f5", "#f3b6c4", "#f3c99b"] as const;
