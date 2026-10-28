use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: String,
    pub path: String,
    pub title: String,
    pub author: String,
    pub format: String,
    pub kind: String,
    pub cover_path: Option<String>,
    pub added_at: i64,
    pub last_opened_at: Option<i64>,
    pub progress: f64,
    pub page_count: i64,
    pub position: Option<Position>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub chapter: i64,
    pub page: i64,
    pub pages_in_chapter: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    pub id: String,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TocItem {
    pub title: String,
    pub chapter: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedBook {
    pub book: Book,
    pub kind: String,
    pub toc: Vec<TocItem>,
    pub chapter_count: i64,
    pub image_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub chapter: i64,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Locator {
    pub chapter: i64,
    pub page: i64,
    pub quote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    pub book_id: String,
    pub kind: String,
    pub locator: Locator,
    pub color: Option<String>,
    pub stamp: Option<String>,
    pub text: Option<String>,
    pub note: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub font_family: String,
    pub font_size: f64,
    pub line_height: f64,
    pub margin: f64,
    pub align: String,
    pub theme: String,
    pub custom_fg: String,
    pub custom_bg: String,
    pub view_mode: String,
    pub spread: bool,
    pub window_width: f64,
    pub window_height: f64,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_family: "Georgia".into(),
            font_size: 20.0,
            line_height: 1.55,
            margin: 48.0,
            align: "left".into(),
            theme: "light".into(),
            custom_fg: "#241c14".into(),
            custom_bg: "#f6f0e6".into(),
            view_mode: "paginated".into(),
            spread: false,
            window_width: 1200.0,
            window_height: 800.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: Vec<Book>,
    pub failed: Vec<ImportFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub lang: String,
    pub name: String,
    pub size_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledDict {
    pub id: String,
    pub lang: String,
    pub name: String,
    pub installed_at: i64,
    pub words: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub word: String,
    pub definition: String,
    pub ipa: Option<String>,
    pub dictionary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictProgress {
    pub id: String,
    pub received: u64,
    pub total: u64,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    pub day: String,
    pub seconds: i64,
    pub pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub today_seconds: i64,
    pub today_pages: i64,
    pub days: Vec<DayStat>,
    pub seconds_per_page: f64,
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
