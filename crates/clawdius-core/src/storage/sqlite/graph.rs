use super::SqliteBackend;
use crate::error::Result;
use crate::graph_rag::ast::{FileInfo, Reference, Relationship, Symbol, SymbolKind};
use crate::storage::backend::GraphRepository;
use crate::storage::error::StorageError;
use rusqlite::{params, OptionalExtension};

impl GraphRepository for SqliteBackend {
    fn insert_file(
        &self,
        file: &FileInfo,
    ) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT OR REPLACE INTO graph_files (path, hash, language, last_modified)
                    VALUES (?1, ?2, ?3, ?4)
                    ",
                    params![file.path, file.hash, file.language, file.last_modified,],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_file".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(conn.last_insert_rowid())
            })
        }
    }

    fn get_file_by_path(
        &self,
        path: &str,
    ) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT id, path, hash, language, last_modified, created_at
                    FROM graph_files WHERE path = ?1
                    ",
                        params![path],
                        |row| {
                            Ok(FileInfo {
                                path: row.get(1)?,
                                hash: row.get(2)?,
                                language: row.get(3)?,
                                last_modified: row.get(4)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_file".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn get_file_by_id(
        &self,
        id: i64,
    ) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT id, path, hash, language, last_modified, created_at
                    FROM graph_files WHERE id = ?1
                    ",
                        params![id],
                        |row| {
                            Ok(FileInfo {
                                path: row.get(1)?,
                                hash: row.get(2)?,
                                language: row.get(3)?,
                                last_modified: row.get(4)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_file by id".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn get_file_id(
        &self,
        path: &str,
    ) -> impl std::future::Future<Output = Result<Option<i64>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        "SELECT id FROM graph_files WHERE path = ?1",
                        params![path],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_file_id".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn delete_file(&self, path: &str) -> impl std::future::Future<Output = Result<bool>> + Send {
        async move {
            self.with_conn(|conn| {
                let affected = conn
                    .execute("DELETE FROM graph_files WHERE path = ?1", params![path])
                    .map_err(|e| StorageError::Query {
                        statement: "DELETE graph_file".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(affected > 0)
            })
        }
    }

    fn count_files(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM graph_files", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT graph_files".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count)
            })
        }
    }

    fn insert_symbol(
        &self,
        symbol: &Symbol,
    ) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT INTO graph_symbols (file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                    ",
                    params![
                        symbol.file_id,
                        symbol.name,
                        format!("{:?}", symbol.kind),
                        symbol.signature,
                        symbol.doc_comment,
                        symbol.start_line,
                        symbol.end_line,
                        symbol.start_col,
                        symbol.end_col,
                    ],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_symbol".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(conn.last_insert_rowid())
            })
        }
    }

    fn find_symbol(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE name = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbol".to_string(),
                        reason: e.to_string(),
                    })?;

                let symbols = stmt
                    .query_map(params![name], |row| {
                        let kind_str: String = row.get(3)?;
                        let kind = match kind_str.as_str() {
                            "Function" => SymbolKind::Function,
                            "Struct" => SymbolKind::Struct,
                            "Enum" => SymbolKind::Enum,
                            "Trait" => SymbolKind::Trait,
                            "Method" => SymbolKind::Method,
                            "Field" => SymbolKind::Field,
                            "Variable" => SymbolKind::Variable,
                            "Module" => SymbolKind::Module,
                            "Interface" => SymbolKind::Interface,
                            "Class" => SymbolKind::Class,
                            _ => SymbolKind::Function,
                        };
                        Ok(Symbol {
                            id: row.get(0)?,
                            file_id: row.get(1)?,
                            name: row.get(2)?,
                            kind,
                            signature: row.get(4)?,
                            doc_comment: row.get(5)?,
                            start_line: row.get(6)?,
                            end_line: row.get(7)?,
                            start_col: row.get(8)?,
                            end_col: row.get(9)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbol".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbol".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(symbols)
            })
        }
    }

    fn find_symbol_by_id(
        &self,
        id: i64,
    ) -> impl std::future::Future<Output = Result<Option<Symbol>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE id = ?1
                    ",
                        params![id],
                        |row| {
                            let kind_str: String = row.get(3)?;
                            let kind = match kind_str.as_str() {
                                "Function" => SymbolKind::Function,
                                "Struct" => SymbolKind::Struct,
                                "Enum" => SymbolKind::Enum,
                                "Trait" => SymbolKind::Trait,
                                "Method" => SymbolKind::Method,
                                "Field" => SymbolKind::Field,
                                "Variable" => SymbolKind::Variable,
                                "Module" => SymbolKind::Module,
                                "Interface" => SymbolKind::Interface,
                                "Class" => SymbolKind::Class,
                                _ => SymbolKind::Function,
                            };
                            Ok(Symbol {
                                id: row.get(0)?,
                                file_id: row.get(1)?,
                                name: row.get(2)?,
                                kind,
                                signature: row.get(4)?,
                                doc_comment: row.get(5)?,
                                start_line: row.get(6)?,
                                end_line: row.get(7)?,
                                start_col: row.get(8)?,
                                end_col: row.get(9)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbol by id".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn find_symbols_by_kind(
        &self,
        kind: &SymbolKind,
    ) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let kind_str = format!("{:?}", kind);
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE kind = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols by kind".to_string(),
                        reason: e.to_string(),
                    })?;

                let symbols = stmt
                    .query_map(params![kind_str], |row| {
                        Ok(Symbol {
                            id: row.get(0)?,
                            file_id: row.get(1)?,
                            name: row.get(2)?,
                            kind: kind.clone(),
                            signature: row.get(4)?,
                            doc_comment: row.get(5)?,
                            start_line: row.get(6)?,
                            end_line: row.get(7)?,
                            start_col: row.get(8)?,
                            end_col: row.get(9)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols by kind".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols by kind".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(symbols)
            })
        }
    }

    fn find_symbols_in_file(
        &self,
        file_id: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE file_id = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols in file".to_string(),
                        reason: e.to_string(),
                    })?;

                let symbols = stmt
                    .query_map(params![file_id], |row| {
                        let kind_str: String = row.get(3)?;
                        let kind = match kind_str.as_str() {
                            "Function" => SymbolKind::Function,
                            "Struct" => SymbolKind::Struct,
                            "Enum" => SymbolKind::Enum,
                            "Trait" => SymbolKind::Trait,
                            "Method" => SymbolKind::Method,
                            "Field" => SymbolKind::Field,
                            "Variable" => SymbolKind::Variable,
                            "Module" => SymbolKind::Module,
                            "Interface" => SymbolKind::Interface,
                            "Class" => SymbolKind::Class,
                            _ => SymbolKind::Function,
                        };
                        Ok(Symbol {
                            id: row.get(0)?,
                            file_id: row.get(1)?,
                            name: row.get(2)?,
                            kind,
                            signature: row.get(4)?,
                            doc_comment: row.get(5)?,
                            start_line: row.get(6)?,
                            end_line: row.get(7)?,
                            start_col: row.get(8)?,
                            end_col: row.get(9)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols in file".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols in file".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(symbols)
            })
        }
    }

    fn search_symbols(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let pattern = format!("%{query}%");
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE name LIKE ?1
                    LIMIT 100
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search graph_symbols".to_string(),
                        reason: e.to_string(),
                    })?;

                let symbols = stmt
                    .query_map(params![pattern], |row| {
                        let kind_str: String = row.get(3)?;
                        let kind = match kind_str.as_str() {
                            "Function" => SymbolKind::Function,
                            "Struct" => SymbolKind::Struct,
                            "Enum" => SymbolKind::Enum,
                            "Trait" => SymbolKind::Trait,
                            "Method" => SymbolKind::Method,
                            "Field" => SymbolKind::Field,
                            "Variable" => SymbolKind::Variable,
                            "Module" => SymbolKind::Module,
                            "Interface" => SymbolKind::Interface,
                            "Class" => SymbolKind::Class,
                            _ => SymbolKind::Function,
                        };
                        Ok(Symbol {
                            id: row.get(0)?,
                            file_id: row.get(1)?,
                            name: row.get(2)?,
                            kind,
                            signature: row.get(4)?,
                            doc_comment: row.get(5)?,
                            start_line: row.get(6)?,
                            end_line: row.get(7)?,
                            start_col: row.get(8)?,
                            end_col: row.get(9)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search graph_symbols".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search graph_symbols".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(symbols)
            })
        }
    }

    fn count_symbols(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM graph_symbols", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT graph_symbols".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count)
            })
        }
    }

    fn delete_symbols_for_file(
        &self,
        file_id: i64,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "DELETE FROM graph_symbols WHERE file_id = ?1",
                    params![file_id],
                )
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_symbols for file".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn insert_reference(
        &self,
        reference: &Reference,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT INTO graph_symbol_refs (symbol_id, file_id, line, col, context)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ",
                    params![
                        reference.symbol_id,
                        reference.file_id,
                        reference.line,
                        reference.col,
                        reference.context,
                    ],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_ref".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn find_symbol_refs(
        &self,
        symbol_id: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Reference>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, symbol_id, file_id, line, col, context
                    FROM graph_symbol_refs WHERE symbol_id = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_refs".to_string(),
                        reason: e.to_string(),
                    })?;

                let refs = stmt
                    .query_map(params![symbol_id], |row| {
                        Ok(Reference {
                            id: row.get(0)?,
                            symbol_id: row.get(1)?,
                            file_id: row.get(2)?,
                            line: row.get(3)?,
                            col: row.get(4)?,
                            context: row.get(5)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_refs".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_refs".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(refs)
            })
        }
    }

    fn count_symbol_refs(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM graph_symbol_refs", [], |row| {
                        row.get(0)
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT graph_symbol_refs".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count)
            })
        }
    }

    fn delete_symbol_refs_for_file(
        &self,
        file_id: i64,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "DELETE FROM graph_symbol_refs WHERE file_id = ?1",
                    params![file_id],
                )
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_refs for file".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn insert_relationship(
        &self,
        relationship: &Relationship,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT INTO graph_relationships (from_symbol, to_symbol, relationship_type)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![
                        relationship.from_symbol,
                        relationship.to_symbol,
                        format!("{:?}", relationship.relationship_type),
                    ],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_relationship".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn find_relationships(
        &self,
        symbol_id: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE from_symbol = ?1 OR to_symbol = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                let rels = stmt
                    .query_map(params![symbol_id], |row| {
                        let rel_type_str: String = row.get(3)?;
                        let rel_type = match rel_type_str.as_str() {
                            "Calls" => crate::graph_rag::ast::RelationshipType::Calls,
                            "Implements" => crate::graph_rag::ast::RelationshipType::Implements,
                            "Contains" => crate::graph_rag::ast::RelationshipType::Contains,
                            "Imports" => crate::graph_rag::ast::RelationshipType::Imports,
                            "References" => crate::graph_rag::ast::RelationshipType::References,
                            _ => crate::graph_rag::ast::RelationshipType::References,
                        };
                        Ok(Relationship {
                            id: row.get(0)?,
                            from_symbol: row.get(1)?,
                            to_symbol: row.get(2)?,
                            relationship_type: rel_type,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(rels)
            })
        }
    }

    fn find_outgoing_relationships(
        &self,
        symbol_id: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE from_symbol = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT outgoing graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                let rels = stmt
                    .query_map(params![symbol_id], |row| {
                        let rel_type_str: String = row.get(3)?;
                        let rel_type = match rel_type_str.as_str() {
                            "Calls" => crate::graph_rag::ast::RelationshipType::Calls,
                            "Implements" => crate::graph_rag::ast::RelationshipType::Implements,
                            "Contains" => crate::graph_rag::ast::RelationshipType::Contains,
                            "Imports" => crate::graph_rag::ast::RelationshipType::Imports,
                            "References" => crate::graph_rag::ast::RelationshipType::References,
                            _ => crate::graph_rag::ast::RelationshipType::References,
                        };
                        Ok(Relationship {
                            id: row.get(0)?,
                            from_symbol: row.get(1)?,
                            to_symbol: row.get(2)?,
                            relationship_type: rel_type,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT outgoing graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT outgoing graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(rels)
            })
        }
    }

    fn find_incoming_relationships(
        &self,
        symbol_id: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE to_symbol = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT incoming graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                let rels = stmt
                    .query_map(params![symbol_id], |row| {
                        let rel_type_str: String = row.get(3)?;
                        let rel_type = match rel_type_str.as_str() {
                            "Calls" => crate::graph_rag::ast::RelationshipType::Calls,
                            "Implements" => crate::graph_rag::ast::RelationshipType::Implements,
                            "Contains" => crate::graph_rag::ast::RelationshipType::Contains,
                            "Imports" => crate::graph_rag::ast::RelationshipType::Imports,
                            "References" => crate::graph_rag::ast::RelationshipType::References,
                            _ => crate::graph_rag::ast::RelationshipType::References,
                        };
                        Ok(Relationship {
                            id: row.get(0)?,
                            from_symbol: row.get(1)?,
                            to_symbol: row.get(2)?,
                            relationship_type: rel_type,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT incoming graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT incoming graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(rels)
            })
        }
    }

    fn count_relationships(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM graph_relationships", [], |row| {
                        row.get(0)
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count)
            })
        }
    }

    fn clear(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute_batch(
                    r"
                    DELETE FROM graph_relationships;
                    DELETE FROM graph_symbol_refs;
                    DELETE FROM graph_symbols;
                    DELETE FROM graph_files;
                    ",
                )
                .map_err(|e| StorageError::Query {
                    statement: "CLEAR graph".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }
}
