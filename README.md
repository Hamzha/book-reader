# Book Reader

A desktop book reader in the spirit of Kindle, built for a PC. It stays on your machine: your books, your place in each book, and your notes live locally and work offline.

The app is not built yet. This repository is the plan for it.

## Stack

**Tauri 2, React (TypeScript), Rust, and SQLite.**

React draws the library, the pages, the themes, and the dictionary popup. Rust is the native core that Tauri uses for windows, files, and the local database. It opens books, detects the format, turns other formats into HTML, stores progress, and looks up words. SQLite keeps the library, reading position, stamps, highlights, and notes on disk.

The reading surface is the browser engine already installed on Windows (WebView2). Reflowable books are HTML, so font controls, themes, text selection, and click-a-word sit in that view. Rust stays a thin core; most of the interface is TypeScript.

## Library

- Import by file picker or by dropping files and folders
- Grid and list views with cover, title, author, format, and progress
- Shelves, search, and sort (recent, title, author, progress)
- Remember the last book and the exact position in every book
- Remove a book from the library while leaving the file on disk

## File types

Reflowable books (EPUB, HTML, Markdown, TXT, DOCX, RTF, MOBI, AZW, AZW3, FB2) are converted to HTML and shown in the reader.

Fixed-layout documents (PDF, DJVU) use a document view, with zoom, fit-width, and fit-page.

Comics (CBZ, CBR, CB7) use an image pager.

Only DRM-free files open here. A store-locked Kindle file stays in Amazon’s app.

## Reading

- Page turns and continuous scroll
- Single page and two-page spread
- Full screen
- Font, size, line height, margins, and alignment
- Light, sepia, dark, and custom colors
- Table of contents and jump to chapter
- Search inside the book
- Progress as percent, page, and time left
- Keyboard shortcuts, plus mouse or trackpad paging

## Dictionary

- A Dictionaries screen to pick a language and download it
- Offline lookup after the download
- Download size, progress, and the option to remove a dictionary
- Click a word on the page to open its meaning
- Pronunciation when the dictionary includes it
- Highlight or add a note from that popup

## Stamps, notes, and marks

Stamps you can drop on a page: star, heart, flag, important, reread, and question. Each stamp shows in the margin and in the notebook.

Also:

- Highlights in several colors
- Notes attached to a passage
- Bookmarks
- One notebook per book that lists stamps, highlights, notes, and bookmarks
- Export that notebook as Markdown or plain text

## Also included

- Text-to-speech for the current page or chapter
- Reading stats: time read, and pages per day
- The app remembers window size, theme, and typography
- It opens into the last book
- Double-click an ebook file to open it in this app

## Build order

1. Local library and import for every format above
2. Reader: paging, scroll, themes, typography, contents, search, and progress
3. Downloadable dictionaries and click-a-word meanings
4. Stamps, highlights, notes, bookmarks, and the notebook
5. Text-to-speech, reading stats, and file association
