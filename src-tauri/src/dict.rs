use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

use crate::model::{CatalogEntry, Definition, DictProgress};

const ENGLISH_URL: &str = "https://raw.githubusercontent.com/adambom/dictionary/master/dictionary.json";
const WIKT_URL: &str = "https://gitlab.com/tdulcet/compact-dictionary/-/raw/main/wiktionary/dictionary-";

struct Pack {
    id: &'static str,
    lang: &'static str,
    name: &'static str,
    size_hint: &'static str,
    /// Wiktionary language code. Empty means the shorter English word list.
    code: &'static str,
}

const PACKS: &[Pack] = &[
    Pack { id: "en", lang: "en", name: "English", size_hint: "about 30 MB", code: "" },
    Pack { id: "en-wikt", lang: "en", name: "English (Wiktionary)", size_hint: "about 125 MB, with pronunciation", code: "en" },
    Pack { id: "ar", lang: "ar", name: "Arabic", size_hint: "about 31 MB, English glosses", code: "ar" },
    Pack { id: "bn", lang: "bn", name: "Bengali", size_hint: "about 2 MB, English glosses", code: "bn" },
    Pack { id: "zh", lang: "zh", name: "Chinese", size_hint: "about 28 MB, English glosses", code: "zh" },
    Pack { id: "cs", lang: "cs", name: "Czech", size_hint: "about 13 MB, English glosses", code: "cs" },
    Pack { id: "da", lang: "da", name: "Danish", size_hint: "about 6 MB, English glosses", code: "da" },
    Pack { id: "nl", lang: "nl", name: "Dutch", size_hint: "about 17 MB, English glosses", code: "nl" },
    Pack { id: "fr", lang: "fr", name: "French", size_hint: "about 45 MB, English glosses", code: "fr" },
    Pack { id: "de", lang: "de", name: "German", size_hint: "about 46 MB, English glosses", code: "de" },
    Pack { id: "el", lang: "el", name: "Greek", size_hint: "about 18 MB, English glosses", code: "el" },
    Pack { id: "he", lang: "he", name: "Hebrew", size_hint: "about 5 MB, English glosses", code: "he" },
    Pack { id: "hi", lang: "hi", name: "Hindi", size_hint: "about 7 MB, English glosses", code: "hi" },
    Pack { id: "hu", lang: "hu", name: "Hungarian", size_hint: "about 41 MB, English glosses", code: "hu" },
    Pack { id: "id", lang: "id", name: "Indonesian", size_hint: "about 4 MB, English glosses", code: "id" },
    Pack { id: "it", lang: "it", name: "Italian", size_hint: "about 59 MB, English glosses", code: "it" },
    Pack { id: "ja", lang: "ja", name: "Japanese", size_hint: "about 26 MB, English glosses", code: "ja" },
    Pack { id: "fa", lang: "fa", name: "Persian", size_hint: "about 2 MB, English glosses", code: "fa" },
    Pack { id: "pl", lang: "pl", name: "Polish", size_hint: "about 40 MB, English glosses", code: "pl" },
    Pack { id: "pt", lang: "pt", name: "Portuguese", size_hint: "about 44 MB, English glosses", code: "pt" },
    Pack { id: "ro", lang: "ro", name: "Romanian", size_hint: "about 21 MB, English glosses", code: "ro" },
    Pack { id: "ru", lang: "ru", name: "Russian", size_hint: "about 103 MB, English glosses", code: "ru" },
    Pack { id: "es", lang: "es", name: "Spanish", size_hint: "about 84 MB, English glosses", code: "es" },
    Pack { id: "sv", lang: "sv", name: "Swedish", size_hint: "about 29 MB, English glosses", code: "sv" },
    Pack { id: "th", lang: "th", name: "Thai", size_hint: "about 3 MB, English glosses", code: "th" },
    Pack { id: "tr", lang: "tr", name: "Turkish", size_hint: "about 48 MB, English glosses", code: "tr" },
    Pack { id: "uk", lang: "uk", name: "Ukrainian", size_hint: "about 20 MB, English glosses", code: "uk" },
    Pack { id: "ur", lang: "ur", name: "Urdu", size_hint: "about 1 MB, English glosses", code: "ur" },
    Pack { id: "vi", lang: "vi", name: "Vietnamese", size_hint: "about 2 MB, English glosses", code: "vi" },
];

pub fn catalog() -> Vec<CatalogEntry> {
    PACKS
        .iter()
        .map(|pack| CatalogEntry {
            id: pack.id.into(),
            lang: pack.lang.into(),
            name: pack.name.into(),
            size_hint: pack.size_hint.into(),
        })
        .collect()
}

fn pack_url(id: &str) -> Result<(&'static Pack, String), String> {
    let pack = PACKS.iter().find(|pack| pack.id == id).ok_or("That dictionary is not in the catalog.")?;
    let url = if pack.code.is_empty() {
        ENGLISH_URL.to_string()
    } else {
        format!("{WIKT_URL}{}.json?inline=false", pack.code)
    };
    Ok((pack, url))
}

pub async fn download(app: &AppHandle, data_dir: &Path, id: &str) -> Result<(PathBuf, i64), String> {
    let (pack, url) = pack_url(id)?;
    let lines = !pack.code.is_empty();
    let dest_dir = data_dir.join("dicts");
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let json_path = dest_dir.join(format!("{id}.json"));
    let db_path = dest_dir.join(format!("{id}.sqlite"));

    let client = reqwest::Client::builder()
        .user_agent("BookReader/0.1")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("The dictionary download failed ({})", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    let mut file = std::fs::File::create(&json_path).map_err(|e| e.to_string())?;
    let mut received = 0u64;
    use std::io::Write;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        received += chunk.len() as u64;
        let _ = app.emit(
            "dict-progress",
            DictProgress {
                id: id.into(),
                received,
                total,
                phase: "download".into(),
            },
        );
    }
    drop(file);

    let _ = app.emit(
        "dict-progress",
        DictProgress {
            id: id.into(),
            received: total,
            total,
            phase: "index".into(),
        },
    );
    let words = if lines {
        let app = app.clone();
        let dict_id = id.to_string();
        let json_path = json_path.clone();
        let db_path = db_path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            index_lines(&json_path, &db_path, |count| {
                let _ = app.emit(
                    "dict-progress",
                    DictProgress {
                        id: dict_id.clone(),
                        received: count,
                        total: 0,
                        phase: "index".into(),
                    },
                );
            })
        })
        .await
        .map_err(|err| err.to_string())??
    } else {
        index_json(&json_path, &db_path)?
    };
    let _ = std::fs::remove_file(&json_path);
    Ok((db_path, words))
}

fn index_json(json_path: &Path, db_path: &Path) -> Result<i64, String> {
    let bytes = std::fs::read(json_path).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("The dictionary file was not valid JSON: {e}"))?;
    let map = value.as_object().ok_or("The dictionary file was not a word list.")?;
    let conn = open_dict(db_path)?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    {
        let mut insert = tx
            .prepare("INSERT OR IGNORE INTO words (word, definition, ipa) VALUES (?1, ?2, NULL)")
            .map_err(|e| e.to_string())?;
        for (word, def) in map {
            let Some(text) = def.as_str() else { continue };
            let word = word.trim();
            if word.is_empty() || text.trim().is_empty() {
                continue;
            }
            insert.execute(rusqlite::params![word, text.trim()]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM words", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(count)
}

fn open_dict(db_path: &Path) -> Result<Connection, String> {
    if db_path.exists() {
        let _ = std::fs::remove_file(db_path);
    }
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "
        CREATE TABLE words (
            id INTEGER PRIMARY KEY,
            word TEXT NOT NULL COLLATE NOCASE UNIQUE,
            definition TEXT NOT NULL,
            ipa TEXT
        );
        CREATE TABLE forms (
            form TEXT NOT NULL COLLATE NOCASE,
            word_id INTEGER NOT NULL
        );
        CREATE INDEX forms_form ON forms(form);
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn gloss(value: &serde_json::Value) -> Option<(String, Option<String>)> {
    let defs: Vec<&str> = value
        .get("d")?
        .as_array()?
        .iter()
        .filter_map(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect();
    if defs.is_empty() {
        return None;
    }
    let pos = value
        .get("p")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let body = defs.join("\n");
    let text = if pos.is_empty() { body } else { format!("{pos}\n{body}") };
    let ipa = value
        .get("i")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    Some((text, ipa))
}

fn index_lines(json_path: &Path, db_path: &Path, mut on_progress: impl FnMut(u64)) -> Result<i64, String> {
    let conn = open_dict(db_path)?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut count = 0u64;
    {
        let mut insert = tx
            .prepare("INSERT OR IGNORE INTO words (word, definition, ipa) VALUES (?1, ?2, ?3)")
            .map_err(|e| e.to_string())?;
        let mut insert_form = tx
            .prepare("INSERT INTO forms (form, word_id) VALUES (?1, ?2)")
            .map_err(|e| e.to_string())?;
        let file = std::fs::File::open(json_path).map_err(|e| e.to_string())?;
        for line in std::io::BufRead::lines(std::io::BufReader::new(file)) {
            let line = line.map_err(|e| e.to_string())?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let Some(word) = value.get("").and_then(|item| item.as_str()).map(str::trim).filter(|item| !item.is_empty()) else {
                continue;
            };
            let Some((definition, ipa)) = gloss(&value) else { continue };
            let changed = insert
                .execute(rusqlite::params![word, definition, ipa])
                .map_err(|e| e.to_string())?;
            if changed == 0 {
                continue;
            }
            let id = tx.last_insert_rowid();
            if let Some(forms) = value.get("f").and_then(|item| item.as_array()) {
                for form in forms {
                    let Some(form) = form.as_str().map(str::trim).filter(|item| !item.is_empty() && *item != word) else {
                        continue;
                    };
                    insert_form.execute(rusqlite::params![form, id]).map_err(|e| e.to_string())?;
                }
            }
            count += 1;
            if count % 10000 == 0 {
                on_progress(count);
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    on_progress(count);
    Ok(count as i64)
}

fn read_hit(conn: &Connection, sql: &str, word: &str) -> Option<(String, String, Option<String>)> {
    conn.query_row(sql, [word], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .ok()
}

fn stems(word: &str) -> Vec<String> {
    let lower = word.to_lowercase();
    if lower.chars().count() < 4 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut push = |stem: &str| {
        if stem.chars().count() >= 3 && !out.iter().any(|item: &String| item == stem) {
            out.push(stem.to_string());
        }
    };
    if let Some(stem) = lower.strip_suffix("ies") {
        push(&format!("{stem}y"));
    }
    for suffix in ["ing", "ed", "es", "s"] {
        if let Some(stem) = lower.strip_suffix(suffix) {
            push(stem);
            if suffix == "ing" || suffix == "ed" {
                push(&format!("{stem}e"));
            }
        }
    }
    out
}

pub fn lookup(paths: &[(String, String)], word: &str) -> Result<Option<Definition>, String> {
    let word = word.trim().trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-');
    if word.is_empty() || word.chars().any(char::is_whitespace) || word.chars().count() > 80 {
        return Ok(None);
    }
    for (name, path) in paths {
        if !Path::new(path).exists() {
            continue;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let hit = read_hit(
            &conn,
            "SELECT word, definition, ipa FROM words WHERE word = ?1 COLLATE NOCASE LIMIT 1",
            word,
        )
        .or_else(|| {
            read_hit(
                &conn,
                "SELECT w.word, w.definition, w.ipa FROM forms f JOIN words w ON w.id = f.word_id WHERE f.form = ?1 COLLATE NOCASE LIMIT 1",
                word,
            )
        })
        .or_else(|| {
            stems(word).into_iter().find_map(|stem| {
                read_hit(
                    &conn,
                    "SELECT word, definition, ipa FROM words WHERE word = ?1 COLLATE NOCASE LIMIT 1",
                    &stem,
                )
            })
        });
        if let Some((found, definition, ipa)) = hit {
            return Ok(Some(Definition {
                word: found,
                definition,
                ipa: ipa.filter(|item| !item.is_empty()),
                dictionary: name.clone(),
            }));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{index_json, index_lines, lookup};

    #[test]
    fn indexes_and_finds_a_word() {
        let dir = std::env::temp_dir().join(format!("book-reader-dict-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let json = dir.join("words.json");
        let db = dir.join("words.sqlite");
        std::fs::write(&json, br#"{"hello":"a greeting","world":"the earth","book":"a set of pages"}"#).unwrap();
        let count = index_json(&json, &db).unwrap();
        assert_eq!(count, 3);
        let paths = [("English".into(), db.to_string_lossy().to_string())];
        let found = lookup(&paths, "hello").unwrap().unwrap();
        assert!(found.definition.contains("greeting"));
        let inflected = lookup(&paths, "books").unwrap().unwrap();
        assert_eq!(inflected.word, "book");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn indexes_a_line_dictionary_and_its_forms() {
        let dir = std::env::temp_dir().join(format!("book-reader-dict-lines-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let json = dir.join("words.json");
        let db = dir.join("words.sqlite");
        std::fs::write(
            &json,
            "{\"\": \"run\", \"p\": [\"verb\"], \"d\": [\"to move quickly\"], \"i\": \"/rʌn/\", \"f\": [\"ran\", \"running\"]}\n{\"\": \"book\", \"p\": [\"noun\"], \"d\": [\"a set of pages\"]}\n",
        )
        .unwrap();
        let count = index_lines(&json, &db, |_| {}).unwrap();
        assert_eq!(count, 2);
        let paths = [("English".into(), db.to_string_lossy().to_string())];
        let ran = lookup(&paths, "ran").unwrap().unwrap();
        assert_eq!(ran.word, "run");
        assert_eq!(ran.ipa.as_deref(), Some("/rʌn/"));
        assert!(ran.definition.contains("move quickly"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
