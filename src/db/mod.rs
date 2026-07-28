//! Unified Database System (Phase 12).
//!
//! Provides a TOML + SQLite hybrid storage system for simulation libraries.
//!
//! # Architecture
//!
//! - **TOML files** (`resources/data/**/*.toml`): store real parameter data
//!   in human-readable/editable format.
//! - **SQLite index** (`resources/db/index.db`): provides fast search,
//!   categorization, versioning, and metadata management over the TOML data.
//! - WAL journal mode: concurrent reads without writer blocking.
//!
//! # Library Categories
//!
//! Supports 15+ library domains covering the full roadmap: material,
//! celestial, fluid, section, electrical, logic gate, chip, board-level,
//! optical, acoustic, chemical, biomolecular, cell, culture media,
//! and semiconductor process parameters.
//!
//! # Usage
//!
//! ```rust,ignore
//! let db = LibraryDb::open("resources/db/index.db")?;
//! let manager = LibraryManager::new(db);
//!
//! // Search for a material
//! let results = manager.search("copper", Some(LibraryCategory::Material))?;
//!
//! // Load entry parameters
//! let copper = manager.load_entry("material/copper")?;
//! ```

use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// ──────────────────────────────────────────────
// 1. Library Category Enum
// ──────────────────────────────────────────────

/// All supported library categories from the roadmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LibraryCategory {
    Material,
    Celestial,
    Fluid,
    Section,
    Electrical,
    LogicGate,
    Chip,
    Board,
    Optical,
    Acoustic,
    Chemical,
    Biomolecule,
    Cell,
    CultureMedia,
    SemiconductorProcess,
}

impl LibraryCategory {
    /// String identifier used as database key.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Material => "material",
            Self::Celestial => "celestial",
            Self::Fluid => "fluid",
            Self::Section => "section",
            Self::Electrical => "electrical",
            Self::LogicGate => "logic_gate",
            Self::Chip => "chip",
            Self::Board => "board",
            Self::Optical => "optical",
            Self::Acoustic => "acoustic",
            Self::Chemical => "chemical",
            Self::Biomolecule => "biomolecule",
            Self::Cell => "cell",
            Self::CultureMedia => "culture_media",
            Self::SemiconductorProcess => "semiconductor_process",
        }
    }

    /// Parse from string (use `parse` via the `FromStr` trait).
    pub fn from_str_name(s: &str) -> Option<Self> {
        match s {
            "material" => Some(Self::Material),
            "celestial" => Some(Self::Celestial),
            "fluid" => Some(Self::Fluid),
            "section" => Some(Self::Section),
            "electrical" => Some(Self::Electrical),
            "logic_gate" => Some(Self::LogicGate),
            "chip" => Some(Self::Chip),
            "board" => Some(Self::Board),
            "optical" => Some(Self::Optical),
            "acoustic" => Some(Self::Acoustic),
            "chemical" => Some(Self::Chemical),
            "biomolecule" => Some(Self::Biomolecule),
            "cell" => Some(Self::Cell),
            "culture_media" => Some(Self::CultureMedia),
            "semiconductor_process" => Some(Self::SemiconductorProcess),
            _ => None,
        }
    }

    /// All categories.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Material,
            Self::Celestial,
            Self::Fluid,
            Self::Section,
            Self::Electrical,
            Self::LogicGate,
            Self::Chip,
            Self::Board,
            Self::Optical,
            Self::Acoustic,
            Self::Chemical,
            Self::Biomolecule,
            Self::Cell,
            Self::CultureMedia,
            Self::SemiconductorProcess,
        ]
    }
}

impl std::str::FromStr for LibraryCategory {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_name(s).ok_or_else(|| format!("unknown library category: {}", s))
    }
}

// ──────────────────────────────────────────────
// 2. Library Entry
// ──────────────────────────────────────────────

/// A single library entry with its parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// Unique entry ID (e.g. "material/copper").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Category.
    pub category: LibraryCategory,
    /// Human-readable description.
    pub description: String,
    /// Arbitrary key-value parameters (stored in TOML).
    pub parameters: HashMap<String, String>,
    /// Tags for search/filtering.
    pub tags: Vec<String>,
    /// Data source (path to TOML file, or "builtin").
    pub source: String,
    /// Version identifier.
    pub version: String,
}

impl LibraryEntry {
    pub fn new(id: &str, name: &str, category: LibraryCategory) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            category,
            description: String::new(),
            parameters: HashMap::new(),
            tags: Vec::new(),
            source: "builtin".to_string(),
            version: "1.0".to_string(),
        }
    }

    /// Get a parameter value parsed as f64.
    pub fn get_scalar(&self, key: &str) -> Option<f64> {
        self.parameters.get(key)?.parse::<f64>().ok()
    }

    /// Get a parameter value as string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.parameters.get(key).map(|s| s.as_str())
    }

    /// Add a parameter.
    pub fn set_param(&mut self, key: &str, value: &str) {
        self.parameters.insert(key.to_string(), value.to_string());
    }

    /// Add a tag.
    pub fn add_tag(&mut self, tag: &str) {
        if !self.tags.contains(&tag.to_string()) {
            self.tags.push(tag.to_string());
        }
    }
}

// ──────────────────────────────────────────────
// 3. Database Configuration
// ──────────────────────────────────────────────

/// Configuration for the library database.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Path to the SQLite index file.
    pub db_path: PathBuf,
    /// Path to the TOML data directory.
    pub data_dir: PathBuf,
    /// WAL mode enabled (default: true).
    pub wal_mode: bool,
    /// Whether to create the database if it doesn't exist.
    pub create_if_missing: bool,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("resources/db/index.db"),
            data_dir: PathBuf::from("resources/data"),
            wal_mode: true,
            create_if_missing: true,
        }
    }
}

// ──────────────────────────────────────────────
// 4. Library Database (SQLite Index)
// ──────────────────────────────────────────────

/// Thread-safe handle to the SQLite library index.
///
/// Uses WAL mode for concurrent read access. Single connection
/// is sufficient for simulation usage patterns (read-heavy, write-rare).
/// The Mutex ensures thread-safe access for the rare write operations.
#[derive(Debug)]
pub struct LibraryDb {
    conn: Mutex<Connection>,
    config: DbConfig,
}

impl LibraryDb {
    /// Open or create the library index database.
    pub fn open(config: DbConfig) -> Result<Self, DbError> {
        let flags = if config.create_if_missing {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };

        let conn = Connection::open_with_flags(&config.db_path, flags)
            .map_err(|e| DbError::ConnectionError(e.to_string()))?;

        // Enable WAL mode for concurrent read performance
        if config.wal_mode {
            conn.execute_batch("PRAGMA journal_mode=WAL;")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
        }

        // Performance pragmas
        conn.execute_batch(
            "PRAGMA synchronous=NORMAL;
             PRAGMA cache_size=-8000;       -- 8 MB cache
             PRAGMA temp_store=MEMORY;
             PRAGMA mmap_size=268435456;     -- 256 MB mmap
             PRAGMA page_size=4096;",
        )
        .map_err(|e| DbError::QueryError(e.to_string()))?;

        let db = Self {
            conn: Mutex::new(conn),
            config,
        };

        db.initialize_schema()?;
        Ok(db)
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &DbConfig {
        &self.config
    }

    // ── Schema Initialization ──

    fn initialize_schema(&self) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS libraries (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                category    TEXT NOT NULL,
                description TEXT DEFAULT '',
                version     TEXT DEFAULT '1.0',
                source      TEXT DEFAULT 'builtin',
                is_public   INTEGER DEFAULT 1,
                created_at  TEXT DEFAULT (datetime('now')),
                updated_at  TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS entry_params (
                entry_id    TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
                key         TEXT NOT NULL,
                value       TEXT NOT NULL,
                value_type  TEXT DEFAULT 'string',
                PRIMARY KEY (entry_id, key)
            );

            CREATE TABLE IF NOT EXISTS entry_tags (
                entry_id    TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
                tag         TEXT NOT NULL,
                PRIMARY KEY (entry_id, tag)
            );

            CREATE INDEX IF NOT EXISTS idx_libraries_category
                ON libraries(category);
            CREATE INDEX IF NOT EXISTS idx_libraries_name
                ON libraries(name);
            CREATE INDEX IF NOT EXISTS idx_entry_params_key
                ON entry_params(key);
            CREATE INDEX IF NOT EXISTS idx_entry_tags_tag
                ON entry_tags(tag);",
        )
        .map_err(|e| DbError::SchemaError(e.to_string()))?;

        // Full-text search virtual table
        let _ = conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS libraries_fts
             USING fts5(id, name, description, tags, category);",
        );

        Ok(())
    }

    // ── Entry CRUD ──

    /// Insert or update a library entry.
    pub fn upsert_entry(&self, entry: &LibraryEntry) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO libraries (id, name, category, description, version, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name, category=excluded.category,
                description=excluded.description, version=excluded.version,
                updated_at=datetime('now')",
            params![
                entry.id,
                entry.name,
                entry.category.as_str(),
                entry.description,
                entry.version,
                entry.source,
            ],
        )
        .map_err(|e| DbError::QueryError(e.to_string()))?;

        // Upsert parameters
        for (key, value) in &entry.parameters {
            conn.execute(
                "INSERT INTO entry_params (entry_id, key, value)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(entry_id, key) DO UPDATE SET value=excluded.value",
                params![entry.id, key, value],
            )
            .map_err(|e| DbError::QueryError(e.to_string()))?;
        }

        // Upsert tags
        for tag in &entry.tags {
            conn.execute(
                "INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?1, ?2)",
                params![entry.id, tag],
            )
            .map_err(|e| DbError::QueryError(e.to_string()))?;
        }

        // Update FTS index (INSERT OR REPLACE for FTS5 compatibility)
        let tags_str = entry.tags.join(" ");
        let _ = conn.execute(
            "INSERT OR REPLACE INTO libraries_fts (id, name, description, tags, category)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                entry.id,
                entry.name,
                entry.description,
                tags_str,
                entry.category.as_str(),
            ],
        );

        Ok(())
    }

    /// Delete an entry by ID.
    pub fn delete_entry(&self, id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute("DELETE FROM libraries WHERE id = ?1", params![id])
            .map_err(|e| DbError::QueryError(e.to_string()))?;
        let _ = conn.execute("DELETE FROM libraries_fts WHERE id = ?1", params![id]);
        Ok(affected > 0)
    }

    /// Load a single entry by ID with all parameters and tags.
    pub fn get_entry(&self, id: &str) -> Result<Option<LibraryEntry>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, category, description, version, source
                 FROM libraries WHERE id = ?1",
            )
            .map_err(|e| DbError::QueryError(e.to_string()))?;

        let result = stmt.query_row(params![id], |row| {
            let cat_str: String = row.get(2)?;
            Ok(LibraryEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                category: LibraryCategory::from_str_name(&cat_str)
                    .ok_or_else(|| rusqlite::Error::InvalidColumnName("category".to_string()))?,
                description: row.get(3)?,
                version: row.get(4)?,
                source: row.get(5)?,
                parameters: HashMap::new(),
                tags: Vec::new(),
            })
        });

        let mut entry = match result {
            Ok(e) => e,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(DbError::QueryError(e.to_string())),
        };

        // Load parameters
        {
            let mut pstmt = conn
                .prepare("SELECT key, value FROM entry_params WHERE entry_id = ?1")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let params_iter = pstmt
                .query_map(params![id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            for p in params_iter.flatten() {
                entry.parameters.insert(p.0, p.1);
            }
        }

        // Load tags
        {
            let mut tstmt = conn
                .prepare("SELECT tag FROM entry_tags WHERE entry_id = ?1 ORDER BY tag")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let tags_iter = tstmt
                .query_map(params![id], |row| row.get::<_, String>(0))
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            for tag in tags_iter.flatten() {
                entry.tags.push(tag);
            }
        }

        Ok(Some(entry))
    }

    /// Search entries by keyword (uses FTS5 full-text search).
    pub fn search(
        &self,
        query: &str,
        category: Option<LibraryCategory>,
    ) -> Result<Vec<LibraryEntry>, DbError> {
        let conn = self.conn.lock().unwrap();
        let category_filter = category.map(|c| c.as_str().to_string());

        // Build search query
        let sql = if category_filter.is_some() {
            "SELECT l.id, l.name, l.category, l.description, l.version, l.source
             FROM libraries l
             INNER JOIN libraries_fts fts ON l.id = fts.id
             WHERE libraries_fts MATCH ?1 AND l.category = ?2
             LIMIT 100"
        } else {
            "SELECT l.id, l.name, l.category, l.description, l.version, l.source
             FROM libraries l
             INNER JOIN libraries_fts fts ON l.id = fts.id
             WHERE libraries_fts MATCH ?1
             LIMIT 100"
        };

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| DbError::QueryError(e.to_string()))?;

        let fts_query = query
            .split_whitespace()
            .map(|w| format!("\"{}\"", w))
            .collect::<Vec<_>>()
            .join(" OR ");

        let entries: Vec<LibraryEntry> = if let Some(ref cat) = category_filter {
            stmt.query_map(params![fts_query, cat], |row| {
                let cat_str: String = row.get(2)?;
                Ok(LibraryEntry {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    category: LibraryCategory::from_str_name(&cat_str)
                        .unwrap_or(LibraryCategory::Material),
                    description: row.get(3)?,
                    version: row.get(4)?,
                    source: row.get(5)?,
                    parameters: HashMap::new(),
                    tags: Vec::new(),
                })
            })
            .map_err(|e| DbError::QueryError(e.to_string()))?
            .flatten()
            .collect()
        } else {
            stmt.query_map(params![fts_query], |row| {
                let cat_str: String = row.get(2)?;
                Ok(LibraryEntry {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    category: LibraryCategory::from_str_name(&cat_str)
                        .unwrap_or(LibraryCategory::Material),
                    description: row.get(3)?,
                    version: row.get(4)?,
                    source: row.get(5)?,
                    parameters: HashMap::new(),
                    tags: Vec::new(),
                })
            })
            .map_err(|e| DbError::QueryError(e.to_string()))?
            .flatten()
            .collect()
        };

        Ok(entries)
    }

    /// List all entries in a category.
    pub fn list_category(&self, category: LibraryCategory) -> Result<Vec<LibraryEntry>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, category, description, version, source
                 FROM libraries WHERE category = ?1
                 ORDER BY name LIMIT 500",
            )
            .map_err(|e| DbError::QueryError(e.to_string()))?;

        let entries = stmt
            .query_map(params![category.as_str()], |row| {
                let cat_str: String = row.get(2)?;
                Ok(LibraryEntry {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    category: LibraryCategory::from_str_name(&cat_str).unwrap_or(category),
                    description: row.get(3)?,
                    version: row.get(4)?,
                    source: row.get(5)?,
                    parameters: HashMap::new(),
                    tags: Vec::new(),
                })
            })
            .map_err(|e| DbError::QueryError(e.to_string()))?
            .flatten()
            .collect();

        Ok(entries)
    }

    /// Get total entry count.
    pub fn entry_count(&self) -> Result<u64, DbError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM libraries", [], |row| row.get(0))
            .map_err(|e| DbError::QueryError(e.to_string()))?;
        Ok(count as u64)
    }
}

// ──────────────────────────────────────────────
// 5. TOML Data Loader
// ──────────────────────────────────────────────

/// Loads library entries from TOML files.
///
/// TOML format expected:
/// ```toml
/// [[entries]]
/// id = "material/copper"
/// name = "Copper"
/// description = "Pure copper, annealed"
/// tags = ["metal", "conductor"]
///
/// [entries.parameters]
/// density = "8960"
/// resistivity = "1.68e-8"
/// thermal_conductivity = "401"
/// ```
#[derive(Debug)]
pub struct TomlLoader {
    data_dir: PathBuf,
}

impl TomlLoader {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Load all TOML files from the data directory.
    pub fn load_all(&self) -> Result<Vec<LibraryEntry>, DbError> {
        let mut all_entries = Vec::new();

        if !self.data_dir.exists() {
            return Ok(all_entries);
        }

        let toml_files =
            std::fs::read_dir(&self.data_dir).map_err(|e| DbError::IoError(e.to_string()))?;

        for entry in toml_files.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                let entries = self.load_file(&path)?;
                all_entries.extend(entries);
            }
        }

        Ok(all_entries)
    }

    /// Load entries from a single TOML file.
    pub fn load_file(&self, path: &Path) -> Result<Vec<LibraryEntry>, DbError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| DbError::IoError(format!("failed to read {:?}: {}", path, e)))?;

        #[derive(Deserialize)]
        struct TomlEntry {
            id: Option<String>,
            name: Option<String>,
            description: Option<String>,
            tags: Option<Vec<String>>,
            parameters: Option<HashMap<String, toml::Value>>,
        }

        #[derive(Deserialize)]
        struct TomlRoot {
            entries: Option<Vec<TomlEntry>>,
            category: Option<String>,
        }

        let root: TomlRoot = toml::from_str(&content)
            .map_err(|e| DbError::ParseError(format!("failed to parse {:?}: {}", path, e)))?;

        let category_str = root.category.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });
        let category =
            LibraryCategory::from_str_name(&category_str).unwrap_or(LibraryCategory::Material);

        let mut entries = Vec::new();
        if let Some(items) = root.entries {
            for item in items {
                let id = item.id.unwrap_or_default();
                let mut entry =
                    LibraryEntry::new(&id, item.name.as_deref().unwrap_or(&id), category);
                entry.description = item.description.unwrap_or_default();
                entry.source = path.to_string_lossy().to_string();

                if let Some(tags) = item.tags {
                    for tag in tags {
                        entry.add_tag(&tag);
                    }
                }

                if let Some(params) = item.parameters {
                    for (k, v) in params {
                        let val_str = match v {
                            toml::Value::String(s) => s,
                            toml::Value::Integer(i) => i.to_string(),
                            toml::Value::Float(f) => f.to_string(),
                            toml::Value::Boolean(b) => b.to_string(),
                            other => other.to_string(),
                        };
                        entry.parameters.insert(k, val_str);
                    }
                }

                entries.push(entry);
            }
        }

        Ok(entries)
    }
}

// ──────────────────────────────────────────────
// 6. Library Manager
// ──────────────────────────────────────────────

/// High-level manager for simulation libraries.
///
/// Combines the SQLite index with TOML data loading and provides
/// a unified API for library operations.
#[derive(Debug)]
pub struct LibraryManager {
    db: LibraryDb,
}

impl LibraryManager {
    /// Create a new library manager with default configuration.
    pub fn new(config: DbConfig) -> Result<Self, DbError> {
        let db = LibraryDb::open(config)?;
        Ok(Self { db })
    }

    /// Open an existing library database.
    pub fn open(db: LibraryDb) -> Self {
        Self { db }
    }

    /// Get a reference to the underlying database.
    pub fn db(&self) -> &LibraryDb {
        &self.db
    }

    /// Import entries from a TOML file into the index.
    pub fn import_toml(&self, path: &Path) -> Result<usize, DbError> {
        let loader = TomlLoader::new(path.parent().unwrap_or(Path::new(".")).to_path_buf());
        let entries = loader.load_file(path)?;
        let count = entries.len();
        for entry in &entries {
            self.db.upsert_entry(entry)?;
        }
        Ok(count)
    }

    /// Import all TOML files from the data directory.
    pub fn import_all(&self) -> Result<usize, DbError> {
        let loader = TomlLoader::new(self.db.config().data_dir.clone());
        let entries = loader.load_all()?;
        let count = entries.len();
        for entry in &entries {
            self.db.upsert_entry(entry)?;
        }
        Ok(count)
    }

    /// Search the library.
    pub fn search(
        &self,
        query: &str,
        category: Option<LibraryCategory>,
    ) -> Result<Vec<LibraryEntry>, DbError> {
        self.db.search(query, category)
    }

    /// Load a specific entry.
    pub fn load_entry(&self, id: &str) -> Result<Option<LibraryEntry>, DbError> {
        self.db.get_entry(id)
    }

    /// List all entries in a category.
    pub fn list(&self, category: LibraryCategory) -> Result<Vec<LibraryEntry>, DbError> {
        self.db.list_category(category)
    }

    /// Add or update an entry.
    pub fn save_entry(&self, entry: &LibraryEntry) -> Result<(), DbError> {
        self.db.upsert_entry(entry)
    }

    /// Remove an entry.
    pub fn remove_entry(&self, id: &str) -> Result<bool, DbError> {
        self.db.delete_entry(id)
    }

    /// Total number of entries.
    pub fn entry_count(&self) -> Result<u64, DbError> {
        self.db.entry_count()
    }
}

// ──────────────────────────────────────────────
// 7. Error Type
// ──────────────────────────────────────────────

/// Database operation errors.
#[derive(Debug)]
pub enum DbError {
    ConnectionError(String),
    QueryError(String),
    SchemaError(String),
    IoError(String),
    ParseError(String),
    NotFound(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionError(msg) => write!(f, "database connection error: {}", msg),
            Self::QueryError(msg) => write!(f, "database query error: {}", msg),
            Self::SchemaError(msg) => write!(f, "schema error: {}", msg),
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
            Self::ParseError(msg) => write!(f, "parse error: {}", msg),
            Self::NotFound(msg) => write!(f, "not found: {}", msg),
        }
    }
}

impl std::error::Error for DbError {}

// ──────────────────────────────────────────────
// 8. Built-in Sample Data
// ──────────────────────────────────────────────

/// Load built-in sample library entries for testing and demonstration.
pub fn load_sample_entries() -> Vec<LibraryEntry> {
    let mut entries = Vec::new();

    // Materials
    let mut copper = LibraryEntry::new("material/copper", "Copper", LibraryCategory::Material);
    copper.description = "Pure copper, annealed".into();
    copper.set_param("density", "8960");
    copper.set_param("resistivity", "1.68e-8");
    copper.set_param("thermal_conductivity", "401");
    copper.set_param("melting_point", "1357");
    copper.add_tag("metal");
    copper.add_tag("conductor");
    copper.add_tag("annealed");
    entries.push(copper);

    let mut silicon = LibraryEntry::new("material/silicon", "Silicon", LibraryCategory::Material);
    silicon.description = "Crystalline silicon, intrinsic".into();
    silicon.set_param("density", "2330");
    silicon.set_param("bandgap", "1.12");
    silicon.set_param("relative_permittivity", "11.7");
    silicon.set_param("melting_point", "1687");
    silicon.add_tag("semiconductor");
    silicon.add_tag("crystalline");
    entries.push(silicon);

    let mut al = LibraryEntry::new("material/aluminum", "Aluminum", LibraryCategory::Material);
    al.description = "Pure aluminum".into();
    al.set_param("density", "2700");
    al.set_param("resistivity", "2.65e-8");
    al.set_param("thermal_conductivity", "237");
    al.set_param("melting_point", "933");
    al.add_tag("metal");
    al.add_tag("conductor");
    entries.push(al);

    // Celestial bodies
    let mut earth = LibraryEntry::new("celestial/earth", "Earth", LibraryCategory::Celestial);
    earth.description = "Earth, third planet from the Sun".into();
    earth.set_param("mass", "5.972e24");
    earth.set_param("radius", "6371000");
    earth.set_param("gravity", "9.80665");
    earth.set_param("orbital_period", "365.25");
    earth.set_param("axial_tilt", "23.44");
    earth.add_tag("planet");
    earth.add_tag("terrestrial");
    entries.push(earth);

    let mut sun = LibraryEntry::new("celestial/sun", "Sun", LibraryCategory::Celestial);
    sun.description = "The Sun, G-type main-sequence star".into();
    sun.set_param("mass", "1.989e30");
    sun.set_param("radius", "696340000");
    sun.set_param("surface_temp", "5778");
    sun.set_param("luminosity", "3.828e26");
    sun.add_tag("star");
    sun.add_tag("G-type");
    entries.push(sun);

    // Electrical components
    let mut resistor = LibraryEntry::new(
        "electrical/resistor",
        "Resistor",
        LibraryCategory::Electrical,
    );
    resistor.description = "Ideal linear resistor".into();
    resistor.set_param("resistance", "1000");
    resistor.set_param("tolerance", "0.01");
    resistor.set_param("power_rating", "0.25");
    resistor.add_tag("passive");
    resistor.add_tag("linear");
    entries.push(resistor);

    let mut capacitor = LibraryEntry::new(
        "electrical/capacitor",
        "Capacitor",
        LibraryCategory::Electrical,
    );
    capacitor.description = "Ideal linear capacitor".into();
    capacitor.set_param("capacitance", "1e-6");
    capacitor.set_param("voltage_rating", "16");
    capacitor.add_tag("passive");
    capacitor.add_tag("linear");
    entries.push(capacitor);

    // Logic gates
    let mut nand = LibraryEntry::new(
        "logic_gate/74LS00",
        "74LS00 Quad NAND",
        LibraryCategory::LogicGate,
    );
    nand.description = "Quad 2-input NAND gate, 74LS family".into();
    nand.set_param("family", "74LS");
    nand.set_param("propagation_delay_ns", "9");
    nand.set_param("power_dissipation_mW", "2");
    nand.set_param("supply_voltage", "5");
    nand.add_tag("TTL");
    nand.add_tag("NAND");
    nand.add_tag("74LS");
    entries.push(nand);

    // Fluids
    let mut water = LibraryEntry::new("fluid/water", "Water", LibraryCategory::Fluid);
    water.description = "Pure water at 20°C, 1 atm".into();
    water.set_param("density", "998.2");
    water.set_param("dynamic_viscosity", "1.0016e-3");
    water.set_param("thermal_conductivity", "0.598");
    water.set_param("specific_heat", "4182");
    water.set_param("bulk_modulus", "2.2e9");
    water.add_tag("liquid");
    water.add_tag("incompressible");
    entries.push(water);

    let mut air = LibraryEntry::new("fluid/air", "Air", LibraryCategory::Fluid);
    air.description = "Dry air at 20°C, 1 atm".into();
    air.set_param("density", "1.204");
    air.set_param("dynamic_viscosity", "1.825e-5");
    air.set_param("thermal_conductivity", "0.0257");
    air.set_param("specific_heat", "1005");
    air.set_param("gas_constant", "287.058");
    air.add_tag("gas");
    air.add_tag("compressible");
    entries.push(air);

    entries
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn setup_test_db() -> LibraryManager {
        let config = DbConfig {
            db_path: PathBuf::from(":memory:"),
            data_dir: PathBuf::from("resources/data"),
            wal_mode: false,
            create_if_missing: true,
        };
        LibraryManager::new(config).unwrap()
    }

    #[test]
    fn test_db_open_in_memory() {
        let mgr = setup_test_db();
        assert_eq!(mgr.entry_count().unwrap(), 0);
    }

    #[test]
    fn test_upsert_and_get_entry() {
        let mgr = setup_test_db();
        let mut entry = LibraryEntry::new("material/test", "TestMat", LibraryCategory::Material);
        entry.set_param("density", "1000");
        entry.add_tag("test");
        mgr.save_entry(&entry).unwrap();

        let loaded = mgr.load_entry("material/test").unwrap().unwrap();
        assert_eq!(loaded.name, "TestMat");
        assert_eq!(loaded.get_str("density"), Some("1000"));
        assert!(loaded.tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_delete_entry() {
        let mgr = setup_test_db();
        let entry = LibraryEntry::new("test/delete_me", "DeleteMe", LibraryCategory::Material);
        mgr.save_entry(&entry).unwrap();
        assert!(mgr.load_entry("test/delete_me").unwrap().is_some());
        mgr.remove_entry("test/delete_me").unwrap();
        assert!(mgr.load_entry("test/delete_me").unwrap().is_none());
    }

    #[test]
    fn test_list_category() {
        let mgr = setup_test_db();
        for i in 0..3 {
            let mut e = LibraryEntry::new(
                &format!("material/test_{}", i),
                &format!("Test_{}", i),
                LibraryCategory::Material,
            );
            e.set_param("value", &i.to_string());
            mgr.save_entry(&e).unwrap();
        }
        let e2 = LibraryEntry::new("celestial/star", "Star", LibraryCategory::Celestial);
        mgr.save_entry(&e2).unwrap();

        let materials = mgr.list(LibraryCategory::Material).unwrap();
        assert_eq!(materials.len(), 3);

        let celestial = mgr.list(LibraryCategory::Celestial).unwrap();
        assert_eq!(celestial.len(), 1);
    }

    #[test]
    fn test_search() {
        let mgr = setup_test_db();
        let mut e = LibraryEntry::new("material/copper", "Copper", LibraryCategory::Material);
        e.description = "High purity copper".into();
        e.add_tag("conductor");
        mgr.save_entry(&e).unwrap();

        let results = mgr.search("copper", None).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.id == "material/copper"));

        let results_cat = mgr
            .search("copper", Some(LibraryCategory::Material))
            .unwrap();
        assert!(!results_cat.is_empty());

        let results_wrong = mgr
            .search("copper", Some(LibraryCategory::Celestial))
            .unwrap();
        assert!(results_wrong.is_empty());
    }

    #[test]
    fn test_sample_data_loading() {
        let mgr = setup_test_db();
        let samples = load_sample_entries();
        assert!(!samples.is_empty());
        for entry in &samples {
            mgr.save_entry(entry).unwrap();
        }
        assert_eq!(mgr.entry_count().unwrap(), samples.len() as u64);

        // Verify specific entry
        let copper = mgr.load_entry("material/copper").unwrap().unwrap();
        assert_eq!(copper.get_str("density"), Some("8960"));
    }

    #[test]
    fn test_library_category_roundtrip() {
        for cat in LibraryCategory::all() {
            let s = cat.as_str();
            let back = LibraryCategory::from_str_name(s).unwrap();
            assert_eq!(cat, back);
        }
        assert!(LibraryCategory::from_str_name("nonexistent").is_none());
    }

    #[test]
    fn test_toml_loader_no_dir() {
        let loader = TomlLoader::new(PathBuf::from("/nonexistent/path"));
        let entries = loader.load_all().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_entry_scalar_access() {
        let mut e = LibraryEntry::new("test/param", "ParamTest", LibraryCategory::Material);
        e.set_param("length", "1.5");
        e.set_param("label", "test_label");
        assert!((e.get_scalar("length").unwrap() - 1.5).abs() < 1e-12);
        assert_eq!(e.get_str("label"), Some("test_label"));
        assert!(e.get_scalar("nonexistent").is_none());
    }

    #[test]
    fn test_entry_update_overwrites() {
        let mgr = setup_test_db();
        let mut e = LibraryEntry::new("test/update", "Original", LibraryCategory::Material);
        e.set_param("value", "1");
        mgr.save_entry(&e).unwrap();

        let mut e2 = LibraryEntry::new("test/update", "Updated", LibraryCategory::Material);
        e2.set_param("value", "2");
        mgr.save_entry(&e2).unwrap();

        let loaded = mgr.load_entry("test/update").unwrap().unwrap();
        assert_eq!(loaded.name, "Updated");
        assert_eq!(loaded.get_str("value"), Some("2"));
    }

    #[test]
    fn test_material_library_entry() {
        let mut e = LibraryEntry::new(
            "material/steel_1018",
            "Steel 1018",
            LibraryCategory::Material,
        );
        e.set_param("density", "7870");
        e.set_param("youngs_modulus", "2.0e11");
        e.set_param("poisson_ratio", "0.29");
        e.set_param("yield_strength", "3.7e8");
        e.add_tag("steel");
        e.add_tag("carbon");

        assert_eq!(e.category, LibraryCategory::Material);
        assert!((e.get_scalar("density").unwrap() - 7870.0).abs() < 1e-12);
        assert!(e.tags.contains(&"steel".to_string()));
    }

    #[test]
    fn test_sample_entries_comprehensive() {
        let samples = load_sample_entries();
        // Verify we have entries from multiple categories
        let categories: std::collections::HashSet<LibraryCategory> =
            samples.iter().map(|e| e.category).collect();
        assert!(categories.len() >= 3, "should span at least 3 categories");

        // All entries should have valid IDs
        for entry in &samples {
            assert!(!entry.id.is_empty(), "entry {} has empty id", entry.name);
            assert!(!entry.name.is_empty(), "entry has empty name");
            // Every entry should have at least one parameter
            assert!(
                !entry.parameters.is_empty() || entry.category == LibraryCategory::LogicGate,
                "entry {} has no identifiable parameters",
                entry.id
            );
        }
    }
}
