use super::PostgresBackend;
use crate::error::Result;
use crate::graph_rag::ast::{
    FileInfo, Reference, Relationship, RelationshipType, Symbol, SymbolKind,
};
use crate::storage::backend::GraphRepository;
use crate::storage::error::StorageError;
use tokio_postgres::types::ToSql;

impl GraphRepository for PostgresBackend {
    fn insert_file(&self, file: &FileInfo) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let result = client
                .query_one(
                    r"
                    INSERT INTO graph_files (path, hash, language, last_modified)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (path) DO UPDATE SET hash = EXCLUDED.hash, language = EXCLUDED.language, last_modified = EXCLUDED.last_modified
                    RETURNING id
                    ",
                    &[
                        &file.path as &(dyn ToSql + Sync),
                        &file.hash,
                        &file.language,
                        &file.last_modified,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_file".to_string(),
                    reason: e.to_string(),
                })?;
            let id: i64 = result.get(0);
            Ok(id)
        }
    }

    fn get_file_by_path(&self, path: &str) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    r"
                    SELECT id, path, hash, language, last_modified, created_at
                    FROM graph_files WHERE path = $1
                    ",
                    &[&path as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_file".to_string(),
                    reason: e.to_string(),
                })?;

            match row {
                Some(r) => Ok(Some(FileInfo {
                    path: r.get(1),
                    hash: r.get(2),
                    language: r.get(3),
                    last_modified: r.get(4),
                })),
                None => Ok(None),
            }
        }
    }

    fn get_file_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    r"
                    SELECT id, path, hash, language, last_modified, created_at
                    FROM graph_files WHERE id = $1
                    ",
                    &[&id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_file by id".to_string(),
                    reason: e.to_string(),
                })?;

            match row {
                Some(r) => Ok(Some(FileInfo {
                    path: r.get(1),
                    hash: r.get(2),
                    language: r.get(3),
                    last_modified: r.get(4),
                })),
                None => Ok(None),
            }
        }
    }

    fn get_file_id(&self, path: &str) -> impl std::future::Future<Output = Result<Option<i64>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    "SELECT id FROM graph_files WHERE path = $1",
                    &[&path as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_file_id".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(row.map(|r| r.get::<_, i64>(0)))
        }
    }

    fn delete_file(&self, path: &str) -> impl std::future::Future<Output = Result<bool>> + Send {
        async move {
            let client = self.get_client().await?;
            let result = client
                .execute(
                    "DELETE FROM graph_files WHERE path = $1",
                    &[&path as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_file".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(result > 0)
        }
    }

    fn count_files(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM graph_files", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT graph_files".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count)
        }
    }

    fn insert_symbol(&self, symbol: &Symbol) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let result = client
                .query_one(
                    r"
                    INSERT INTO graph_symbols (file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    RETURNING id
                    ",
                    &[
                        &symbol.file_id as &(dyn ToSql + Sync),
                        &symbol.name,
                        &format!("{:?}", symbol.kind),
                        &symbol.signature,
                        &symbol.doc_comment,
                        &symbol.start_line,
                        &symbol.end_line,
                        &symbol.start_col,
                        &symbol.end_col,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_symbol".to_string(),
                    reason: e.to_string(),
                })?;
            let id: i64 = result.get(0);
            Ok(id)
        }
    }

    fn find_symbol(&self, name: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE name = $1
                    ",
                    &[&name as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_symbol".to_string(),
                    reason: e.to_string(),
                })?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(symbols)
        }
    }

    fn find_symbol_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE id = $1
                    ",
                    &[&id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_symbol by id".to_string(),
                    reason: e.to_string(),
                })?;

            match row {
                Some(r) => Ok(Some(Self::row_to_symbol(&r)?)),
                None => Ok(None),
            }
        }
    }

    fn find_symbols_by_kind(&self, kind: &SymbolKind) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let kind_str = format!("{:?}", kind);
            let rows = client
                .query(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE kind = $1
                    ",
                    &[&kind_str as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_symbols by kind".to_string(),
                    reason: e.to_string(),
                })?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(symbols)
        }
    }

    fn find_symbols_in_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE file_id = $1
                    ",
                    &[&file_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_symbols in file".to_string(),
                    reason: e.to_string(),
                })?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(symbols)
        }
    }

    fn search_symbols(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let pattern = format!("%{query}%");
            let rows = client
                .query(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE name LIKE $1
                    LIMIT 100
                    ",
                    &[&pattern as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT search graph_symbols".to_string(),
                    reason: e.to_string(),
                })?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(symbols)
        }
    }

    fn count_symbols(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM graph_symbols", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT graph_symbols".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count)
        }
    }

    fn delete_symbols_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM graph_symbols WHERE file_id = $1",
                    &[&file_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_symbols for file".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn insert_reference(&self, reference: &Reference) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    r"
                    INSERT INTO graph_symbol_refs (symbol_id, file_id, line, col, context)
                    VALUES ($1, $2, $3, $4, $5)
                    ",
                    &[
                        &reference.symbol_id as &(dyn ToSql + Sync),
                        &reference.file_id,
                        &reference.line,
                        &reference.col,
                        &reference.context,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_ref".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn find_symbol_refs(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Reference>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, symbol_id, file_id, line, col, context
                    FROM graph_symbol_refs WHERE symbol_id = $1
                    ",
                    &[&symbol_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_refs".to_string(),
                    reason: e.to_string(),
                })?;

            let refs: Vec<Reference> = rows.iter().map(Self::row_to_reference).collect();
            Ok(refs)
        }
    }

    fn count_symbol_refs(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM graph_symbol_refs", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT graph_symbol_refs".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count)
        }
    }

    fn delete_symbol_refs_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM graph_symbol_refs WHERE file_id = $1",
                    &[&file_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_refs for file".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn insert_relationship(&self, relationship: &Relationship) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    r"
                    INSERT INTO graph_relationships (from_symbol, to_symbol, relationship_type)
                    VALUES ($1, $2, $3)
                    ",
                    &[
                        &relationship.from_symbol as &(dyn ToSql + Sync),
                        &relationship.to_symbol,
                        &format!("{:?}", relationship.relationship_type),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_relationship".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn find_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE from_symbol = $1 OR to_symbol = $1
                    ",
                    &[&symbol_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_relationships".to_string(),
                    reason: e.to_string(),
                })?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn find_outgoing_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE from_symbol = $1
                    ",
                    &[&symbol_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT outgoing graph_relationships".to_string(),
                    reason: e.to_string(),
                })?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn find_incoming_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE to_symbol = $1
                    ",
                    &[&symbol_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT incoming graph_relationships".to_string(),
                    reason: e.to_string(),
                })?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn count_relationships(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM graph_relationships", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT graph_relationships".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count)
        }
    }

    fn clear(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .batch_execute(
                    r"
                    DELETE FROM graph_relationships;
                    DELETE FROM graph_symbol_refs;
                    DELETE FROM graph_symbols;
                    DELETE FROM graph_files;
                    ",
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "CLEAR graph".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }
}
