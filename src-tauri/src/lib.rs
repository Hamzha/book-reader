mod db;
mod dict;
mod formats;
mod htmlutil;
mod import;
mod model;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::db::Db;
use crate::model::*;

struct AppState {
    db: Mutex<Db>,
    data_dir: PathBuf,
    pending: Mutex<Vec<String>>,
}

#[tauri::command]
fn library_list(state: State<AppState>, sort: String, query: String, shelf_id: Option<String>) -> Result<Vec<Book>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_books(&sort, &query, shelf_id.as_deref())
}

#[tauri::command]
fn shelf_list(state: State<AppState>) -> Result<Vec<Shelf>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_shelves()
}

#[tauri::command]
fn shelf_create(state: State<AppState>, name: String) -> Result<Shelf, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.create_shelf(&name)
}

#[tauri::command]
fn shelf_delete(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_shelf(&id)
}

#[tauri::command]
fn shelf_add_book(state: State<AppState>, shelf_id: String, book_id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_to_shelf(&shelf_id, &book_id)
}

#[tauri::command]
fn shelf_remove_book(state: State<AppState>, shelf_id: String, book_id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.remove_from_shelf(&shelf_id, &book_id)
}

#[tauri::command]
fn import_paths(app: AppHandle, state: State<AppState>, paths: Vec<String>) -> Result<ImportReport, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let report = import::import_paths(&db, &state.data_dir, &paths);
    let _ = app.emit("library-changed", ());
    Ok(report)
}

#[tauri::command]
fn remove_book(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(package) = db.remove_book(&id)? {
        let _ = std::fs::remove_dir_all(package);
    }
    Ok(())
}

#[tauri::command]
fn open_book(state: State<AppState>, id: String) -> Result<OpenedBook, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let book = db.get_book(&id)?.ok_or_else(|| "That book is not in the library.".to_string())?;
    db.touch_open(&id)?;
    let package = import::package_of(&db, &id)?;
    let toc = import::load_toc(&package);
    let chapter_count = match book.kind.as_str() {
        "reflow" => import::chapter_count(&package).max(book.page_count),
        _ => book.page_count,
    };
    let image_count = if book.kind == "comic" {
        import::image_count(&package)
    } else {
        0
    };
    let kind = book.kind.clone();
    let book = db.get_book(&id)?.unwrap_or(book);
    Ok(OpenedBook {
        book,
        kind,
        toc,
        chapter_count,
        image_count,
    })
}

#[tauri::command]
fn chapter_html(state: State<AppState>, book_id: String, index: i64) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    import::chapter_html(&db, &book_id, index)
}

#[tauri::command]
fn save_position(state: State<AppState>, book_id: String, position: Position, progress: f64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.save_position(&book_id, &position, progress)
}

#[tauri::command]
fn search_book(state: State<AppState>, book_id: String, query: String) -> Result<Vec<SearchHit>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    import::search_book(&db, &book_id, &query)
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<Appearance, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.appearance()
}

#[tauri::command]
fn save_settings(state: State<AppState>, appearance: Appearance) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.save_appearance(&appearance)
}

#[tauri::command]
fn dict_catalog() -> Vec<CatalogEntry> {
    dict::catalog()
}

#[tauri::command]
fn dict_installed(state: State<AppState>) -> Result<Vec<InstalledDict>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_dicts()
}

#[tauri::command]
async fn dict_download(app: AppHandle, id: String) -> Result<InstalledDict, String> {
    let data_dir = app.state::<AppState>().data_dir.clone();
    let (path, words) = dict::download(&app, &data_dir, &id).await?;
    let entry = dict::catalog().into_iter().find(|e| e.id == id).ok_or("Missing catalog entry")?;
    let state = app.state::<AppState>();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.upsert_dict(&entry.id, &entry.lang, &entry.name, &path.to_string_lossy(), words)?;
    Ok(InstalledDict {
        id: entry.id,
        lang: entry.lang,
        name: entry.name,
        installed_at: now_ms(),
        words,
    })
}

#[tauri::command]
fn dict_remove(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(path) = db.remove_dict(&id)? {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[tauri::command]
fn dict_lookup(state: State<AppState>, word: String) -> Result<Option<Definition>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let paths = db.dict_paths()?;
    drop(db);
    dict::lookup(&paths, &word)
}

#[tauri::command]
fn annotations_list(state: State<AppState>, book_id: String) -> Result<Vec<Annotation>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_annotations(&book_id)
}

#[tauri::command]
fn annotation_add(
    state: State<AppState>,
    book_id: String,
    kind: String,
    locator: Locator,
    color: Option<String>,
    stamp: Option<String>,
    text: Option<String>,
    note: Option<String>,
) -> Result<Annotation, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_annotation(&book_id, &kind, &locator, color, stamp, text, note)
}

#[tauri::command]
fn annotation_delete(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_annotation(&id)
}

#[tauri::command]
fn annotations_export(state: State<AppState>, book_id: String, format: String, path: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let text = import::export_notebook(&db, &book_id, &format)?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[tauri::command]
fn stats_summary(state: State<AppState>) -> Result<Stats, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.stats()
}

#[tauri::command]
fn tick_reading(state: State<AppState>, book_id: String, seconds: i64, pages: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.tick(&book_id, seconds, pages)
}

#[tauri::command]
fn take_launch_files(state: State<AppState>) -> Result<Vec<String>, String> {
    let mut pending = state.pending.lock().map_err(|e| e.to_string())?;
    Ok(std::mem::take(&mut *pending))
}

#[tauri::command]
fn book_by_path(state: State<AppState>, path: String) -> Result<Option<Book>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let key = import::normalize(Path::new(&path));
    let Some(id) = db.find_by_path_key(&key)? else {
        return Ok(None);
    };
    db.get_book(&id)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let files = launch_files(args);
            let _ = app.emit("open-files", files);
        }))
        .register_uri_scheme_protocol("book", |ctx, request| serve_book(ctx.app_handle(), request))
        .setup(|app| {
            let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
            let db = Db::open(&data_dir.join("library.sqlite"))?;
            let pending = launch_files(std::env::args().collect());
            app.manage(AppState {
                db: Mutex::new(db),
                data_dir,
                pending: Mutex::new(pending),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            library_list,
            shelf_list,
            shelf_create,
            shelf_delete,
            shelf_add_book,
            shelf_remove_book,
            import_paths,
            remove_book,
            open_book,
            chapter_html,
            save_position,
            search_book,
            get_settings,
            save_settings,
            dict_catalog,
            dict_installed,
            dict_download,
            dict_remove,
            dict_lookup,
            annotations_list,
            annotation_add,
            annotation_delete,
            annotations_export,
            stats_summary,
            tick_reading,
            take_launch_files,
            book_by_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running Book Reader");
}

fn launch_files(args: Vec<String>) -> Vec<String> {
    args.into_iter()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .filter(|arg| Path::new(arg).exists())
        .collect()
}

fn serve_book(app: &AppHandle, request: tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let path = request.uri().path().to_string();
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let result = (|| -> Result<(Vec<u8>, &'static str), String> {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        match parts.as_slice() {
            ["cover", id] => {
                let book = db.get_book(id)?.ok_or("missing")?;
                let path = book.cover_path.ok_or("missing")?;
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                Ok((bytes, mime_of(&path)))
            }
            ["pdf", id] => {
                let path = db.source_path(id)?.ok_or("missing")?;
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                Ok((bytes, "application/pdf"))
            }
            ["asset", id, name] => {
                let package = import::package_of(&db, id)?;
                let file = safe_join(&package.join("images"), name)?;
                let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
                Ok((bytes, mime_of(file.to_str().unwrap_or(name))))
            }
            ["comic", id, index] => {
                let package = import::package_of(&db, id)?;
                let index: usize = index.parse().map_err(|_| "bad page".to_string())?;
                let file = image_at(&package.join("images"), index)?;
                let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
                Ok((bytes, mime_of(file.to_str().unwrap_or(""))))
            }
            ["djvu", id, index] => {
                let source = db.source_path(id)?.ok_or("missing")?;
                let index: usize = index.parse().map_err(|_| "bad page".to_string())?;
                let package = import::package_of(&db, id)?;
                drop(db);
                let cache = package.join("djvu");
                std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
                let file = cache.join(format!("{index:04}.png"));
                if !file.exists() {
                    let png = formats::djvu::render_page(Path::new(&source), index)?;
                    std::fs::write(&file, &png).map_err(|e| e.to_string())?;
                    return Ok((png, "image/png"));
                }
                let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
                Ok((bytes, "image/png"))
            }
            _ => Err("not found".into()),
        }
    })();
    match result {
        Ok((body, mime)) => response(200, mime, body),
        Err(_) => response(404, "text/plain", b"Not found".to_vec()),
    }
}

fn image_at(dir: &Path, index: usize) -> Result<PathBuf, String> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    files.get(index).cloned().ok_or_else(|| "missing image".into())
}

fn safe_join(base: &Path, name: &str) -> Result<PathBuf, String> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err("bad path".into());
    }
    Ok(base.join(name))
}

fn mime_of(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else if lower.ends_with(".pdf") {
        "application/pdf"
    } else {
        "application/octet-stream"
    }
}

fn response(status: u16, mime: &str, body: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "private, max-age=3600")
        .body(body)
        .unwrap()
}
