//! SQLite-based graph store for code knowledge

use crate::error::Result;
use crate::graph_rag::ast::{
    FileInfo, Reference, Relationship, RelationshipType, Symbol, SymbolKind,
};
use rusqlite::{params, Connection};
use std::path::Path;

const SCHEMA_VERSION: i32 = 1;

const SCHEMA_SQL: &str = r"
    -- Files table
    CREATE TABLE IF NOT EXISTS files (
        id INTEGER PRIMARY KEY,
        path TEXT UNIQUE NOT NULL,
        hash TEXT NOT NULL,
        language TEXT,
        last_modified TIMESTAMP,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

    -- Symbols table
    CREATE TABLE IF NOT EXISTS symbols (
        id INTEGER PRIMARY KEY,
        file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        kind TEXT NOT NULL,
        signature TEXT,
        doc_comment TEXT,
        start_line INTEGER,
        end_line INTEGER,
        start_col INTEGER,
        end_col INTEGER,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

    -- Symbol symbol_refs table (symbol usage)
    CREATE TABLE IF NOT EXISTS symbol_refs (
        id INTEGER PRIMARY KEY,
        symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
        file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
        line INTEGER,
        col INTEGER,
        context TEXT,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

    -- Relationships table
    CREATE TABLE IF NOT EXISTS relationships (
        id INTEGER PRIMARY KEY,
        from_symbol INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
        to_symbol INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
        relationship_type TEXT NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

    -- Schema version table
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER PRIMARY KEY
    );

    -- Indexes
    CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
    CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
    CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
    CREATE INDEX IF NOT EXISTS idx_refs_symbol ON symbol_refs(symbol_id);
    CREATE INDEX IF NOT EXISTS idx_refs_file ON symbol_refs(file_id);
    CREATE INDEX IF NOT EXISTS idx_relationships_from ON relationships(from_symbol);
    CREATE INDEX IF NOT EXISTS idx_relationships_to ON relationships(to_symbol);
    CREATE INDEX IF NOT EXISTS idx_relationships_type ON relationships(relationship_type);
";

pub struct GraphStore {
    conn: Connection,
}

impl GraphStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = if path.as_os_str().is_empty() {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let store = Self { conn };
        store.initialize_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::open(Path::new(""))
    }

    fn initialize_schema(&self) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        self.conn.execute_batch(SCHEMA_SQL)?;

        let version: Option<i32> = self
            .conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .ok();

        if version.is_none() {
            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn insert_file(&self, file: &FileInfo) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO files (path, hash, language, last_modified) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET hash = ?2, language = ?3, last_modified = ?4",
            params![file.path, file.hash, file.language, file.last_modified],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_file_by_path(&self, path: &str) -> Result<Option<FileInfo>> {
        let result = self.conn.query_row(
            "SELECT id, path, hash, language, last_modified FROM files WHERE path = ?1",
            params![path],
            |row| {
                Ok(FileInfo {
                    path: row.get(1)?,
                    hash: row.get(2)?,
                    language: row.get(3)?,
                    last_modified: row.get(4)?,
                })
            },
        );

        match result {
            Ok(file) => Ok(Some(file)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_file_by_id(&self, id: i64) -> Result<Option<FileInfo>> {
        let result = self.conn.query_row(
            "SELECT id, path, hash, language, last_modified FROM files WHERE id = ?1",
            params![id],
            |row| {
                Ok(FileInfo {
                    path: row.get(1)?,
                    hash: row.get(2)?,
                    language: row.get(3)?,
                    last_modified: row.get(4)?,
                })
            },
        );

        match result {
            Ok(file) => Ok(Some(file)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_file_id(&self, path: &str) -> Result<Option<i64>> {
        let result = self.conn.query_row(
            "SELECT id FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        );

        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete_file(&self, path: &str) -> Result<bool> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM files WHERE path = ?1", params![path])?;
        Ok(rows_affected > 0)
    }

    pub fn insert_symbol(&self, symbol: &Symbol) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO symbols (file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                symbol.file_id,
                symbol.name,
                symbol.kind.as_str(),
                symbol.signature,
                symbol.doc_comment,
                symbol.start_line,
                symbol.end_line,
                symbol.start_col,
                symbol.end_col
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_reference(&self, reference: &Reference) -> Result<()> {
        self.conn.execute(
            "INSERT INTO symbol_refs (symbol_id, file_id, line, col, context)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                reference.symbol_id,
                reference.file_id,
                reference.line,
                reference.col,
                reference.context
            ],
        )?;
        Ok(())
    }

    pub fn insert_relationship(&self, relationship: &Relationship) -> Result<()> {
        self.conn.execute(
            "INSERT INTO relationships (from_symbol, to_symbol, relationship_type)
             VALUES (?1, ?2, ?3)",
            params![
                relationship.from_symbol,
                relationship.to_symbol,
                relationship.relationship_type.as_str()
            ],
        )?;
        Ok(())
    }

    pub fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
             FROM symbols WHERE name = ?1"
        )?;

        let symbols = stmt
            .query_map(params![name], |row| {
                Ok(Symbol {
                    id: Some(row.get(0)?),
                    file_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: SymbolKind::from_str(&row.get::<_, String>(3)?),
                    signature: row.get(4)?,
                    doc_comment: row.get(5)?,
                    start_line: row.get(6)?,
                    end_line: row.get(7)?,
                    start_col: row.get(8)?,
                    end_col: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(symbols)
    }

    pub fn find_symbol_by_id(&self, id: i64) -> Result<Option<Symbol>> {
        let result = self.conn.query_row(
            "SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
             FROM symbols WHERE id = ?1",
            params![id],
            |row| {
                Ok(Symbol {
                    id: Some(row.get(0)?),
                    file_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: SymbolKind::from_str(&row.get::<_, String>(3)?),
                    signature: row.get(4)?,
                    doc_comment: row.get(5)?,
                    start_line: row.get(6)?,
                    end_line: row.get(7)?,
                    start_col: row.get(8)?,
                    end_col: row.get(9)?,
                })
            },
        );

        match result {
            Ok(symbol) => Ok(Some(symbol)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn find_symbol_refs(&self, symbol_id: i64) -> Result<Vec<Reference>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, symbol_id, file_id, line, col, context
             FROM symbol_refs WHERE symbol_id = ?1",
        )?;

        let refs = stmt
            .query_map(params![symbol_id], |row| {
                Ok(Reference {
                    id: Some(row.get(0)?),
                    symbol_id: row.get(1)?,
                    file_id: row.get(2)?,
                    line: row.get(3)?,
                    col: row.get(4)?,
                    context: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(refs)
    }

    pub fn find_relationships(&self, symbol_id: i64) -> Result<Vec<Relationship>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_symbol, to_symbol, relationship_type
             FROM relationships WHERE from_symbol = ?1 OR to_symbol = ?1",
        )?;

        let rels = stmt
            .query_map(params![symbol_id], |row| {
                Ok(Relationship {
                    id: Some(row.get(0)?),
                    from_symbol: row.get(1)?,
                    to_symbol: row.get(2)?,
                    relationship_type: RelationshipType::from_str(&row.get::<_, String>(3)?),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rels)
    }

    pub fn find_outgoing_relationships(&self, symbol_id: i64) -> Result<Vec<Relationship>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_symbol, to_symbol, relationship_type
             FROM relationships WHERE from_symbol = ?1",
        )?;

        let rels = stmt
            .query_map(params![symbol_id], |row| {
                Ok(Relationship {
                    id: Some(row.get(0)?),
                    from_symbol: row.get(1)?,
                    to_symbol: row.get(2)?,
                    relationship_type: RelationshipType::from_str(&row.get::<_, String>(3)?),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rels)
    }

    pub fn find_incoming_relationships(&self, symbol_id: i64) -> Result<Vec<Relationship>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_symbol, to_symbol, relationship_type
             FROM relationships WHERE to_symbol = ?1",
        )?;

        let rels = stmt
            .query_map(params![symbol_id], |row| {
                Ok(Relationship {
                    id: Some(row.get(0)?),
                    from_symbol: row.get(1)?,
                    to_symbol: row.get(2)?,
                    relationship_type: RelationshipType::from_str(&row.get::<_, String>(3)?),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rels)
    }

    pub fn search_symbols(&self, query: &str) -> Result<Vec<Symbol>> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
             FROM symbols WHERE name LIKE ?1 OR signature LIKE ?1 OR doc_comment LIKE ?1"
        )?;

        let symbols = stmt
            .query_map(params![pattern], |row| {
                Ok(Symbol {
                    id: Some(row.get(0)?),
                    file_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: SymbolKind::from_str(&row.get::<_, String>(3)?),
                    signature: row.get(4)?,
                    doc_comment: row.get(5)?,
                    start_line: row.get(6)?,
                    end_line: row.get(7)?,
                    start_col: row.get(8)?,
                    end_col: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(symbols)
    }

    pub fn find_symbols_by_kind(&self, kind: &SymbolKind) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
             FROM symbols WHERE kind = ?1"
        )?;

        let symbols = stmt
            .query_map(params![kind.as_str()], |row| {
                Ok(Symbol {
                    id: Some(row.get(0)?),
                    file_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: SymbolKind::from_str(&row.get::<_, String>(3)?),
                    signature: row.get(4)?,
                    doc_comment: row.get(5)?,
                    start_line: row.get(6)?,
                    end_line: row.get(7)?,
                    start_col: row.get(8)?,
                    end_col: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(symbols)
    }

    pub fn find_symbols_in_file(&self, file_id: i64) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
             FROM symbols WHERE file_id = ?1"
        )?;

        let symbols = stmt
            .query_map(params![file_id], |row| {
                Ok(Symbol {
                    id: Some(row.get(0)?),
                    file_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: SymbolKind::from_str(&row.get::<_, String>(3)?),
                    signature: row.get(4)?,
                    doc_comment: row.get(5)?,
                    start_line: row.get(6)?,
                    end_line: row.get(7)?,
                    start_col: row.get(8)?,
                    end_col: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(symbols)
    }

    pub fn clear(&self) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        self.conn.execute("DELETE FROM relationships", [])?;
        self.conn.execute("DELETE FROM symbol_refs", [])?;
        self.conn.execute("DELETE FROM symbols", [])?;
        self.conn.execute("DELETE FROM files", [])?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_symbols_for_file(&self, file_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])?;
        Ok(())
    }

    pub fn delete_symbol_refs_for_file(&self, file_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM symbol_refs WHERE file_id = ?1",
            params![file_id],
        )?;
        Ok(())
    }

    pub fn count_files(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn count_symbols(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn count_symbol_refs(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbol_refs", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn count_relationships(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_store() -> GraphStore {
        GraphStore::open_in_memory().expect("Failed to create test store")
    }

    fn create_test_file(store: &GraphStore) -> i64 {
        let file = FileInfo {
            path: "test.rs".to_string(),
            hash: "abc123".to_string(),
            language: Some("rust".to_string()),
            last_modified: Some(1234567890),
        };
        store.insert_file(&file).expect("Failed to insert file")
    }

    #[test]
    fn test_schema_creation() {
        let store = create_test_store();
        assert!(store.count_files().is_ok());
    }

    #[test]
    fn test_insert_file() {
        let store = create_test_store();
        let file = FileInfo {
            path: "src/main.rs".to_string(),
            hash: "hash123".to_string(),
            language: Some("rust".to_string()),
            last_modified: Some(1000),
        };

        let id = store.insert_file(&file).expect("Failed to insert file");
        assert!(id > 0);

        let retrieved = store
            .get_file_by_path("src/main.rs")
            .expect("Failed to get file");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.path, "src/main.rs");
        assert_eq!(retrieved.hash, "hash123");
        assert_eq!(retrieved.language, Some("rust".to_string()));
    }

    #[test]
    fn test_insert_file_upsert() {
        let store = create_test_store();
        let file = FileInfo {
            path: "src/lib.rs".to_string(),
            hash: "hash1".to_string(),
            language: Some("rust".to_string()),
            last_modified: Some(1000),
        };

        let id1 = store.insert_file(&file).expect("Failed to insert file");

        let file_updated = FileInfo {
            path: "src/lib.rs".to_string(),
            hash: "hash2".to_string(),
            language: Some("rust".to_string()),
            last_modified: Some(2000),
        };

        let id2 = store
            .insert_file(&file_updated)
            .expect("Failed to upsert file");
        assert_eq!(id1, id2);

        let retrieved = store
            .get_file_by_path("src/lib.rs")
            .expect("Failed to get file");
        assert_eq!(retrieved.unwrap().hash, "hash2");
    }

    #[test]
    fn test_insert_symbol() {
        let store = create_test_store();
        let file_id = create_test_file(&store);

        let symbol = Symbol {
            id: None,
            file_id,
            name: "my_function".to_string(),
            kind: SymbolKind::Function,
            signature: Some("fn my_function(x: i32) -> i32".to_string()),
            doc_comment: Some("This is a test function".to_string()),
            start_line: 10,
            end_line: 15,
            start_col: 1,
            end_col: 2,
        };

        let id = store
            .insert_symbol(&symbol)
            .expect("Failed to insert symbol");
        assert!(id > 0);

        let found = store
            .find_symbol("my_function")
            .expect("Failed to find symbol");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "my_function");
        assert_eq!(found[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_insert_reference() {
        let store = create_test_store();
        let file_id = create_test_file(&store);

        let symbol = Symbol {
            id: None,
            file_id,
            name: "helper".to_string(),
            kind: SymbolKind::Function,
            signature: None,
            doc_comment: None,
            start_line: 1,
            end_line: 5,
            start_col: 1,
            end_col: 1,
        };
        let symbol_id = store
            .insert_symbol(&symbol)
            .expect("Failed to insert symbol");

        let reference = Reference {
            id: None,
            symbol_id,
            file_id,
            line: 20,
            col: 5,
            context: Some("let result = helper(data);".to_string()),
        };

        store
            .insert_reference(&reference)
            .expect("Failed to insert reference");

        let refs = store
            .find_symbol_refs(symbol_id)
            .expect("Failed to find symbol_refs");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].line, 20);
        assert_eq!(
            refs[0].context,
            Some("let result = helper(data);".to_string())
        );
    }

    #[test]
    fn test_insert_relationship() {
        let store = create_test_store();
        let file_id = create_test_file(&store);

        let symbol1 = Symbol {
            id: None,
            file_id,
            name: "caller".to_string(),
            kind: SymbolKind::Function,
            signature: None,
            doc_comment: None,
            start_line: 1,
            end_line: 5,
            start_col: 1,
            end_col: 1,
        };
        let from_id = store
            .insert_symbol(&symbol1)
            .expect("Failed to insert symbol1");

        let symbol2 = Symbol {
            id: None,
            file_id,
            name: "callee".to_string(),
            kind: SymbolKind::Function,
            signature: None,
            doc_comment: None,
            start_line: 10,
            end_line: 15,
            start_col: 1,
            end_col: 1,
        };
        let to_id = store
            .insert_symbol(&symbol2)
            .expect("Failed to insert symbol2");

        let relationship = Relationship {
            id: None,
            from_symbol: from_id,
            to_symbol: to_id,
            relationship_type: RelationshipType::Calls,
        };

        store
            .insert_relationship(&relationship)
            .expect("Failed to insert relationship");

        let rels = store
            .find_relationships(from_id)
            .expect("Failed to find relationships");
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].from_symbol, from_id);
        assert_eq!(rels[0].to_symbol, to_id);
        assert_eq!(rels[0].relationship_type, RelationshipType::Calls);
    }

    #[test]
    fn test_search_symbols() {
        let store = create_test_store();
        let file_id = create_test_file(&store);

        let symbol = Symbol {
            id: None,
            file_id,
            name: "process_data".to_string(),
            kind: SymbolKind::Function,
            signature: Some("fn process_data(input: &str) -> String".to_string()),
            doc_comment: Some("Process the input data".to_string()),
            start_line: 1,
            end_line: 5,
            start_col: 1,
            end_col: 1,
        };
        store
            .insert_symbol(&symbol)
            .expect("Failed to insert symbol");

        let results = store.search_symbols("process").expect("Failed to search");
        assert!(!results.is_empty());

        let results = store.search_symbols("input").expect("Failed to search");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_find_symbols_by_kind() {
        let store = create_test_store();
        let file_id = create_test_file(&store);

        let func = Symbol {
            id: None,
            file_id,
            name: "func".to_string(),
            kind: SymbolKind::Function,
            signature: None,
            doc_comment: None,
            start_line: 1,
            end_line: 5,
            start_col: 1,
            end_col: 1,
        };
        store
            .insert_symbol(&func)
            .expect("Failed to insert function");

        let struct_sym = Symbol {
            id: None,
            file_id,
            name: "MyStruct".to_string(),
            kind: SymbolKind::Struct,
            signature: None,
            doc_comment: None,
            start_line: 10,
            end_line: 15,
            start_col: 1,
            end_col: 1,
        };
        store
            .insert_symbol(&struct_sym)
            .expect("Failed to insert struct");

        let functions = store
            .find_symbols_by_kind(&SymbolKind::Function)
            .expect("Failed to find by kind");
        assert_eq!(functions.len(), 1);

        let structs = store
            .find_symbols_by_kind(&SymbolKind::Struct)
            .expect("Failed to find by kind");
        assert_eq!(structs.len(), 1);
    }

    #[test]
    fn test_delete_file_cascades() {
        let store = create_test_store();
        let file = FileInfo {
            path: "to_delete.rs".to_string(),
            hash: "hash".to_string(),
            language: Some("rust".to_string()),
            last_modified: None,
        };
        let file_id = store.insert_file(&file).expect("Failed to insert file");

        let symbol = Symbol {
            id: None,
            file_id,
            name: "to_delete_func".to_string(),
            kind: SymbolKind::Function,
            signature: None,
            doc_comment: None,
            start_line: 1,
            end_line: 5,
            start_col: 1,
            end_col: 1,
        };
        store
            .insert_symbol(&symbol)
            .expect("Failed to insert symbol");

        assert_eq!(store.count_symbols().unwrap(), 1);

        store
            .delete_file("to_delete.rs")
            .expect("Failed to delete file");

        assert_eq!(store.count_files().unwrap(), 0);
        assert_eq!(store.count_symbols().unwrap(), 0);
    }

    #[test]
    fn test_persistence() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let path = temp_file.path();

        {
            let store = GraphStore::open(path).expect("Failed to open store");
            let file = FileInfo {
                path: "persist.rs".to_string(),
                hash: "hash".to_string(),
                language: Some("rust".to_string()),
                last_modified: None,
            };
            store.insert_file(&file).expect("Failed to insert file");
        }

        {
            let store = GraphStore::open(path).expect("Failed to reopen store");
            let file = store
                .get_file_by_path("persist.rs")
                .expect("Failed to get file");
            assert!(file.is_some());
        }
    }

    #[test]
    fn test_counts() {
        let store = create_test_store();

        assert_eq!(store.count_files().unwrap(), 0);
        assert_eq!(store.count_symbols().unwrap(), 0);
        assert_eq!(store.count_symbol_refs().unwrap(), 0);
        assert_eq!(store.count_relationships().unwrap(), 0);

        let file_id = create_test_file(&store);
        assert_eq!(store.count_files().unwrap(), 1);

        let symbol = Symbol {
            id: None,
            file_id,
            name: "test".to_string(),
            kind: SymbolKind::Function,
            signature: None,
            doc_comment: None,
            start_line: 1,
            end_line: 5,
            start_col: 1,
            end_col: 1,
        };
        let symbol_id = store.insert_symbol(&symbol).unwrap();
        assert_eq!(store.count_symbols().unwrap(), 1);

        let reference = Reference {
            id: None,
            symbol_id,
            file_id,
            line: 10,
            col: 5,
            context: None,
        };
        store.insert_reference(&reference).unwrap();
        assert_eq!(store.count_symbol_refs().unwrap(), 1);
    }

    #[test]
    fn test_symbol_kind_roundtrip() {
        assert_eq!(
            SymbolKind::from_str(SymbolKind::Function.as_str()),
            SymbolKind::Function
        );
        assert_eq!(
            SymbolKind::from_str(SymbolKind::Struct.as_str()),
            SymbolKind::Struct
        );
        assert_eq!(
            SymbolKind::from_str("custom"),
            SymbolKind::Other("custom".to_string())
        );
    }

    #[test]
    fn test_relationship_type_roundtrip() {
        assert_eq!(
            RelationshipType::from_str(RelationshipType::Calls.as_str()),
            RelationshipType::Calls
        );
        assert_eq!(
            RelationshipType::from_str(RelationshipType::Extends.as_str()),
            RelationshipType::Extends
        );
        assert_eq!(
            RelationshipType::from_str("custom"),
            RelationshipType::Other("custom".to_string())
        );
    }
}
