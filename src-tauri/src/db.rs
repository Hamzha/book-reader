use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::*;

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS books (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                path_key TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                author TEXT NOT NULL,
                format TEXT NOT NULL,
                kind TEXT NOT NULL,
                cover_path TEXT,
                added_at INTEGER NOT NULL,
                last_opened_at INTEGER,
                position_json TEXT,
                progress REAL NOT NULL DEFAULT 0,
                page_count INTEGER NOT NULL DEFAULT 0,
                package_dir TEXT
            );
            CREATE TABLE IF NOT EXISTS shelves (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS book_shelves (
                book_id TEXT NOT NULL,
                shelf_id TEXT NOT NULL,
                PRIMARY KEY (book_id, shelf_id)
            );
            CREATE TABLE IF NOT EXISTS annotations (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                locator_json TEXT NOT NULL,
                color TEXT,
                stamp TEXT,
                text TEXT,
                note TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS dictionaries (
                id TEXT PRIMARY KEY,
                lang TEXT NOT NULL,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                installed_at INTEGER NOT NULL,
                words INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS reading_sessions (
                day TEXT NOT NULL,
                book_id TEXT NOT NULL,
                seconds INTEGER NOT NULL,
                pages INTEGER NOT NULL,
                PRIMARY KEY (day, book_id)
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }

    pub fn list_books(&self, sort: &str, query: &str, shelf_id: Option<&str>) -> Result<Vec<Book>, String> {
        let order = match sort {
            "title" => "title COLLATE NOCASE ASC",
            "author" => "author COLLATE NOCASE ASC",
            "progress" => "progress DESC",
            _ => "COALESCE(last_opened_at, added_at) DESC",
        };
        let like = format!("%{}%", query.trim());
        let mut sql = format!(
            "SELECT id, path, title, author, format, kind, cover_path, added_at, last_opened_at, position_json, progress, page_count
             FROM books WHERE (title LIKE ?1 OR author LIKE ?1)"
        );
        if shelf_id.is_some() {
            sql.push_str(" AND id IN (SELECT book_id FROM book_shelves WHERE shelf_id = ?2)");
        }
        sql.push_str(" ORDER BY ");
        sql.push_str(order);
        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = if let Some(shelf) = shelf_id {
            stmt.query_map(params![like, shelf], map_book)
                .map_err(|e| e.to_string())?
        } else {
            stmt.query_map(params![like], map_book)
                .map_err(|e| e.to_string())?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn get_book(&self, id: &str) -> Result<Option<Book>, String> {
        self.conn
            .query_row(
                "SELECT id, path, title, author, format, kind, cover_path, added_at, last_opened_at, position_json, progress, page_count FROM books WHERE id = ?1",
                [id],
                map_book,
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn find_by_path_key(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row("SELECT id FROM books WHERE path_key = ?1", [key], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn package_dir(&self, id: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row("SELECT package_dir FROM books WHERE id = ?1", [id], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn source_path(&self, id: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row("SELECT path FROM books WHERE id = ?1", [id], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn upsert_book(&self, book: &NewBook) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO books (id, path, path_key, title, author, format, kind, cover_path, added_at, last_opened_at, position_json, progress, page_count, package_dir)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL, 0, ?10, ?11)
                 ON CONFLICT(path_key) DO UPDATE SET
                    path = excluded.path,
                    title = excluded.title,
                    author = excluded.author,
                    format = excluded.format,
                    kind = excluded.kind,
                    cover_path = excluded.cover_path,
                    page_count = excluded.page_count,
                    package_dir = excluded.package_dir",
                params![
                    book.id,
                    book.path,
                    book.path_key,
                    book.title,
                    book.author,
                    book.format,
                    book.kind,
                    book.cover_path,
                    book.added_at,
                    book.page_count,
                    book.package_dir
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove_book(&self, id: &str) -> Result<Option<String>, String> {
        let package = self.package_dir(id)?;
        self.conn.execute("DELETE FROM annotations WHERE book_id = ?1", [id]).ok();
        self.conn.execute("DELETE FROM book_shelves WHERE book_id = ?1", [id]).ok();
        self.conn.execute("DELETE FROM reading_sessions WHERE book_id = ?1", [id]).ok();
        self.conn.execute("DELETE FROM books WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        Ok(package)
    }

    pub fn touch_open(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE books SET last_opened_at = ?2 WHERE id = ?1",
                params![id, now_ms()],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_position(&self, id: &str, position: &Position, progress: f64) -> Result<(), String> {
        let json = serde_json::to_string(position).map_err(|e| e.to_string())?;
        self.conn
            .execute(
                "UPDATE books SET position_json = ?2, progress = ?3, last_opened_at = ?4 WHERE id = ?1",
                params![id, json, progress.clamp(0.0, 1.0), now_ms()],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_shelves(&self) -> Result<Vec<Shelf>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.name, COUNT(bs.book_id) FROM shelves s
                 LEFT JOIN book_shelves bs ON bs.shelf_id = s.id
                 GROUP BY s.id ORDER BY s.name COLLATE NOCASE",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Shelf {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    count: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn create_shelf(&self, name: &str) -> Result<Shelf, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("A shelf needs a name.".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.conn
            .execute("INSERT INTO shelves (id, name) VALUES (?1, ?2)", params![id, name])
            .map_err(|e| {
                if e.to_string().contains("UNIQUE") {
                    "A shelf with that name already exists.".to_string()
                } else {
                    e.to_string()
                }
            })?;
        Ok(Shelf {
            id,
            name: name.into(),
            count: 0,
        })
    }

    pub fn delete_shelf(&self, id: &str) -> Result<(), String> {
        self.conn.execute("DELETE FROM book_shelves WHERE shelf_id = ?1", [id]).ok();
        self.conn.execute("DELETE FROM shelves WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn add_to_shelf(&self, shelf_id: &str, book_id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO book_shelves (book_id, shelf_id) VALUES (?1, ?2)",
                params![book_id, shelf_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove_from_shelf(&self, shelf_id: &str, book_id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM book_shelves WHERE book_id = ?1 AND shelf_id = ?2",
                params![book_id, shelf_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_annotations(&self, book_id: &str) -> Result<Vec<Annotation>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, book_id, kind, locator_json, color, stamp, text, note, created_at
                 FROM annotations WHERE book_id = ?1 ORDER BY created_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([book_id], map_annotation).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn add_annotation(&self, book_id: &str, kind: &str, locator: &Locator, color: Option<String>, stamp: Option<String>, text: Option<String>, note: Option<String>) -> Result<Annotation, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = now_ms();
        let locator_json = serde_json::to_string(locator).map_err(|e| e.to_string())?;
        self.conn
            .execute(
                "INSERT INTO annotations (id, book_id, kind, locator_json, color, stamp, text, note, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![id, book_id, kind, locator_json, color, stamp, text, note, created_at],
            )
            .map_err(|e| e.to_string())?;
        Ok(Annotation {
            id,
            book_id: book_id.into(),
            kind: kind.into(),
            locator: locator.clone(),
            color,
            stamp,
            text,
            note,
            created_at,
        })
    }

    pub fn delete_annotation(&self, id: &str) -> Result<(), String> {
        self.conn.execute("DELETE FROM annotations WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn appearance(&self) -> Result<Appearance, String> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = 'appearance'", [], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(raw
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    pub fn save_appearance(&self, appearance: &Appearance) -> Result<(), String> {
        let json = serde_json::to_string(appearance).map_err(|e| e.to_string())?;
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES ('appearance', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [json],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_dicts(&self) -> Result<Vec<InstalledDict>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, lang, name, installed_at, words FROM dictionaries ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(InstalledDict {
                    id: row.get(0)?,
                    lang: row.get(1)?,
                    name: row.get(2)?,
                    installed_at: row.get(3)?,
                    words: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn upsert_dict(&self, id: &str, lang: &str, name: &str, path: &str, words: i64) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO dictionaries (id, lang, name, path, installed_at, words)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET path = excluded.path, installed_at = excluded.installed_at, words = excluded.words",
                params![id, lang, name, path, now_ms(), words],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn dict_path(&self, id: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row("SELECT path FROM dictionaries WHERE id = ?1", [id], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn remove_dict(&self, id: &str) -> Result<Option<String>, String> {
        let path = self.dict_path(id)?;
        self.conn.execute("DELETE FROM dictionaries WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn dict_paths(&self) -> Result<Vec<(String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, path FROM dictionaries")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn tick(&self, book_id: &str, seconds: i64, pages: i64) -> Result<(), String> {
        let day = today();
        self.conn
            .execute(
                "INSERT INTO reading_sessions (day, book_id, seconds, pages) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(day, book_id) DO UPDATE SET
                    seconds = seconds + excluded.seconds,
                    pages = pages + excluded.pages",
                params![day, book_id, seconds.max(0), pages.max(0)],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn stats(&self) -> Result<Stats, String> {
        let day = today();
        let (today_seconds, today_pages): (i64, i64) = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(seconds), 0), COALESCE(SUM(pages), 0) FROM reading_sessions WHERE day = ?1",
                [day],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, 0));
        let mut stmt = self
            .conn
            .prepare(
                "SELECT day, SUM(seconds), SUM(pages) FROM reading_sessions
                 GROUP BY day ORDER BY day DESC LIMIT 7",
            )
            .map_err(|e| e.to_string())?;
        let days = stmt
            .query_map([], |row| {
                Ok(DayStat {
                    day: row.get(0)?,
                    seconds: row.get(1)?,
                    pages: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let (seconds, pages): (i64, i64) = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(seconds), 0), COALESCE(SUM(pages), 0) FROM reading_sessions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, 0));
        let seconds_per_page = if pages > 0 {
            seconds as f64 / pages as f64
        } else {
            45.0
        };
        Ok(Stats {
            today_seconds,
            today_pages,
            days,
            seconds_per_page,
        })
    }

}

pub struct NewBook {
    pub id: String,
    pub path: String,
    pub path_key: String,
    pub title: String,
    pub author: String,
    pub format: String,
    pub kind: String,
    pub cover_path: Option<String>,
    pub added_at: i64,
    pub page_count: i64,
    pub package_dir: String,
}

fn map_book(row: &rusqlite::Row<'_>) -> rusqlite::Result<Book> {
    let position_json: Option<String> = row.get(9)?;
    Ok(Book {
        id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        author: row.get(3)?,
        format: row.get(4)?,
        kind: row.get(5)?,
        cover_path: row.get(6)?,
        added_at: row.get(7)?,
        last_opened_at: row.get(8)?,
        position: position_json.and_then(|s| serde_json::from_str(&s).ok()),
        progress: row.get(10)?,
        page_count: row.get(11)?,
    })
}

fn map_annotation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Annotation> {
    let locator_json: String = row.get(3)?;
    let locator = serde_json::from_str(&locator_json).unwrap_or(Locator {
        chapter: 0,
        page: 0,
        quote: None,
    });
    Ok(Annotation {
        id: row.get(0)?,
        book_id: row.get(1)?,
        kind: row.get(2)?,
        locator,
        color: row.get(4)?,
        stamp: row.get(5)?,
        text: row.get(6)?,
        note: row.get(7)?,
        created_at: row.get(8)?,
    })
}
