//! Cross-backend storage test suite.
//!
//! Defines a comprehensive set of tests that can be run against any
//! `StorageBackend` implementation. Use the `backend_test_suite!` macro
//! to generate tests for a specific backend.

/// Macro that generates a full test suite for a storage backend.
///
/// # Arguments
///
/// * `$module_name` — Name for the generated test module
/// * `$backend_type` — The `StorageBackend` type
/// * `$make:block` — Async block that returns `(backend, cleanup_fn)` where
///   backend is fully initialized (migrated) and cleanup_fn is an optional
///   async cleanup. For backends that need no cleanup, return `|| async {}`.
macro_rules! backend_test_suite {
    ($module_name:ident, $backend_type:ty, $make:block) => {
        mod $module_name {
            use crate::graph_rag::ast::{
                FileInfo, Relationship, RelationshipType, Symbol, SymbolKind,
            };
            use crate::session::types::{Message, Session, TokenUsage};
            use crate::storage::{GraphRepository, SessionRepository, StorageBackend, TimelineRepository};
            use crate::timeline::FileChangeType;
            use std::path::PathBuf;

            /// Helper: create a fresh, migrated backend.
            async fn make_backend() -> $backend_type {
                $make
            }

            // ═══════════════════════════════════════════════════
            // SessionRepository tests
            // ═══════════════════════════════════════════════════

            #[tokio::test]
            async fn test_session_crud() {
                let backend = make_backend().await;

                let mut session = Session::new();
                session.title = Some("Test Session".to_string());
                session.meta.provider = Some("anthropic".to_string());
                session.meta.model = Some("claude-3-5-sonnet".to_string());
                backend.create_session(&session).await.unwrap();

                let loaded = backend
                    .load_session(&session.id)
                    .await
                    .unwrap()
                    .expect("session should exist");
                assert_eq!(loaded.title, Some("Test Session".to_string()));
                assert_eq!(loaded.meta.provider, Some("anthropic".to_string()));
                assert!(loaded.messages.is_empty());

                let msg = Message::user("Hello, world!");
                backend.save_message(&session.id, &msg).await.unwrap();

                let full = backend
                    .load_session_full(&session.id)
                    .await
                    .unwrap()
                    .expect("session should exist");
                assert_eq!(full.messages.len(), 1);
                assert_eq!(full.messages[0].as_text(), Some("Hello, world!"));

                let sessions = backend.list_sessions().await.unwrap();
                assert_eq!(sessions.len(), 1);

                backend.delete_session(&session.id).await.unwrap();
                let sessions = backend.list_sessions().await.unwrap();
                assert!(sessions.is_empty());

                let deleted = backend.load_session(&session.id).await.unwrap();
                assert!(deleted.is_none());
            }

            #[tokio::test]
            async fn test_save_and_search_messages() {
                let backend = make_backend().await;

                let session = Session::new();
                backend.create_session(&session).await.unwrap();

                backend
                    .save_message(&session.id, &Message::user("find me if you can"))
                    .await
                    .unwrap();
                backend
                    .save_message(&session.id, &Message::assistant("I found you"))
                    .await
                    .unwrap();

                let results = backend.search_messages("find me").await.unwrap();
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].0, session.id);
            }

            #[tokio::test]
            async fn test_update_token_usage() {
                let backend = make_backend().await;

                let session = Session::new();
                let id = session.id;
                backend.create_session(&session).await.unwrap();

                backend
                    .update_token_usage(
                        &id,
                        &TokenUsage {
                            input: 100,
                            output: 50,
                            cached: 10,
                        },
                    )
                    .await
                    .unwrap();

                let loaded = backend.load_session(&id).await.unwrap().unwrap();
                assert_eq!(loaded.token_usage.input, 100);
                assert_eq!(loaded.token_usage.output, 50);
                assert_eq!(loaded.token_usage.cached, 10);
            }

            #[tokio::test]
            async fn test_list_sessions_ordered_by_updated_at() {
                let backend = make_backend().await;

                let s1 = Session::new();
                let s2 = Session::new();
                backend.create_session(&s1).await.unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                backend.create_session(&s2).await.unwrap();

                let sessions = backend.list_sessions().await.unwrap();
                assert_eq!(sessions.len(), 2);
                assert_eq!(sessions[0].id, s2.id);
                assert_eq!(sessions[1].id, s1.id);
            }

            #[tokio::test]
            async fn test_delete_session_cascades_messages() {
                let backend = make_backend().await;

                let session = Session::new();
                backend.create_session(&session).await.unwrap();
                backend
                    .save_message(&session.id, &Message::user("will be deleted"))
                    .await
                    .unwrap();

                backend.delete_session(&session.id).await.unwrap();
                let results = backend.search_messages("deleted").await.unwrap();
                assert!(results.is_empty());
            }

            #[tokio::test]
            async fn test_multiple_sessions() {
                let backend = make_backend().await;

                let s1 = Session::new();
                let s2 = Session::new();
                let s3 = Session::new();

                backend.create_session(&s1).await.unwrap();
                backend.create_session(&s2).await.unwrap();
                backend.create_session(&s3).await.unwrap();

                assert_eq!(backend.list_sessions().await.unwrap().len(), 3);

                backend.delete_session(&s2.id).await.unwrap();
                assert_eq!(backend.list_sessions().await.unwrap().len(), 2);
            }

            // ═══════════════════════════════════════════════════
            // TimelineRepository tests
            // ═══════════════════════════════════════════════════

            #[tokio::test]
            async fn test_checkpoint_crud() {
                let backend = make_backend().await;

                let id = backend.create_checkpoint("v1", Some("first version")).await.unwrap();

                let checkpoints = backend.list_checkpoints().await.unwrap();
                assert_eq!(checkpoints.len(), 1);
                assert_eq!(checkpoints[0].name, "v1");

                let cp = backend.get_checkpoint(&id).await.unwrap().expect("checkpoint exists");
                assert_eq!(cp.name, "v1");
                assert_eq!(backend.checkpoint_count().await.unwrap(), 1);

                backend.delete_checkpoint(&id).await.unwrap();
                assert_eq!(backend.checkpoint_count().await.unwrap(), 0);
            }

            #[tokio::test]
            async fn test_cleanup_old_checkpoints() {
                let backend = make_backend().await;

                backend.create_checkpoint("cp1", None).await.unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                backend.create_checkpoint("cp2", None).await.unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                backend.create_checkpoint("cp3", None).await.unwrap();

                let deleted = backend.cleanup_old_checkpoints(1).await.unwrap();
                assert_eq!(deleted, 2);
                assert_eq!(backend.checkpoint_count().await.unwrap(), 1);

                let remaining = backend.list_checkpoints().await.unwrap();
                assert_eq!(remaining[0].name, "cp3");
            }

            #[tokio::test]
            async fn test_track_file() {
                let backend = make_backend().await;

                let path = PathBuf::from("/test/file.rs");
                backend.track_file(&path).await.unwrap();
                backend.track_file(&path).await.unwrap(); // idempotent

                assert_eq!(backend.tracked_file_count().await.unwrap(), 1);
            }

            #[tokio::test]
            async fn test_storage_stats() {
                let backend = make_backend().await;

                let stats = backend.storage_stats().await.unwrap();
                assert_eq!(stats.checkpoint_count, 0);
                assert_eq!(stats.tracked_file_count, 0);

                backend.create_checkpoint("v1", None).await.unwrap();
                backend.track_file(&PathBuf::from("/test.rs")).await.unwrap();

                let stats = backend.storage_stats().await.unwrap();
                assert_eq!(stats.checkpoint_count, 1);
                assert_eq!(stats.tracked_file_count, 1);
            }

            #[tokio::test]
            async fn test_query_by_name() {
                let backend = make_backend().await;

                backend.create_checkpoint("release-1.0", None).await.unwrap();
                backend.create_checkpoint("release-2.0", None).await.unwrap();
                backend.create_checkpoint("hotfix", None).await.unwrap();

                assert_eq!(backend.query_by_name("release").await.unwrap().len(), 2);
                assert_eq!(backend.query_by_name("hotfix").await.unwrap().len(), 1);
            }

            #[tokio::test]
            async fn test_export_import_checkpoint() {
                let backend = make_backend().await;

                let id = backend.create_checkpoint("export-test", None).await.unwrap();

                let exported = backend.export_checkpoint(&id).await.unwrap();
                assert_eq!(exported.name, "export-test");

                backend.delete_checkpoint(&id).await.unwrap();
                assert_eq!(backend.checkpoint_count().await.unwrap(), 0);

                let new_id = backend.import_checkpoint(exported).await.unwrap();
                assert_eq!(backend.checkpoint_count().await.unwrap(), 1);

                let restored = backend.get_checkpoint(&new_id).await.unwrap().unwrap();
                assert_eq!(restored.name, "export-test");
            }

            // ═══════════════════════════════════════════════════
            // GraphRepository tests
            // ═══════════════════════════════════════════════════

            #[tokio::test]
            async fn test_graph_file_crud() {
                let backend = make_backend().await;

                let file = FileInfo {
                    path: "src/main.rs".to_string(),
                    hash: "abc123".to_string(),
                    language: Some("Rust".to_string()),
                    last_modified: None,
                };

                let id = backend.insert_file(&file).await.unwrap();

                let found = backend.get_file_by_path("src/main.rs").await.unwrap();
                assert!(found.is_some());
                assert_eq!(found.unwrap().hash, "abc123");

                let found = backend.get_file_by_id(id).await.unwrap();
                assert!(found.is_some());
                assert_eq!(backend.count_files().await.unwrap(), 1);

                let deleted = backend.delete_file("src/main.rs").await.unwrap();
                assert!(deleted);
                assert_eq!(backend.count_files().await.unwrap(), 0);
            }

            #[tokio::test]
            async fn test_graph_symbol_crud() {
                let backend = make_backend().await;

                let file = FileInfo {
                    path: "src/lib.rs".to_string(),
                    hash: "def456".to_string(),
                    language: Some("Rust".to_string()),
                    last_modified: None,
                };
                let file_id = backend.insert_file(&file).await.unwrap();

                let symbol = Symbol {
                    id: None,
                    file_id,
                    name: "main".to_string(),
                    kind: SymbolKind::Function,
                    signature: Some("fn main()".to_string()),
                    doc_comment: None,
                    start_line: 1,
                    end_line: 10,
                    start_col: 0,
                    end_col: 15,
                };
                let sym_id = backend.insert_symbol(&symbol).await.unwrap();

                assert_eq!(backend.find_symbol("main").await.unwrap().len(), 1);

                let found = backend.find_symbol_by_id(sym_id).await.unwrap();
                assert!(found.is_some());
                assert_eq!(found.unwrap().name, "main");

                assert_eq!(backend.find_symbols_by_kind(&SymbolKind::Function).await.unwrap().len(), 1);
                assert_eq!(backend.find_symbols_in_file(file_id).await.unwrap().len(), 1);
                assert_eq!(backend.count_symbols().await.unwrap(), 1);
                assert_eq!(backend.search_symbols("mai").await.unwrap().len(), 1);
            }

            #[tokio::test]
            async fn test_graph_relationships() {
                let backend = make_backend().await;

                let file = FileInfo {
                    path: "src/mod.rs".to_string(),
                    hash: "rel789".to_string(),
                    language: Some("Rust".to_string()),
                    last_modified: None,
                };
                let file_id = backend.insert_file(&file).await.unwrap();

                let sym_a = Symbol {
                    id: None,
                    file_id,
                    name: "function_a".to_string(),
                    kind: SymbolKind::Function,
                    signature: None,
                    doc_comment: None,
                    start_line: 1,
                    end_line: 5,
                    start_col: 0,
                    end_col: 20,
                };
                let sym_b = Symbol {
                    id: None,
                    file_id,
                    name: "function_b".to_string(),
                    kind: SymbolKind::Function,
                    signature: None,
                    doc_comment: None,
                    start_line: 10,
                    end_line: 15,
                    start_col: 0,
                    end_col: 20,
                };
                let id_a = backend.insert_symbol(&sym_a).await.unwrap();
                let id_b = backend.insert_symbol(&sym_b).await.unwrap();

                let rel = Relationship {
                    id: None,
                    from_symbol: id_a,
                    to_symbol: id_b,
                    relationship_type: RelationshipType::Calls,
                };
                backend.insert_relationship(&rel).await.unwrap();

                assert_eq!(backend.find_relationships(id_a).await.unwrap().len(), 1);
                assert_eq!(backend.find_outgoing_relationships(id_a).await.unwrap().len(), 1);
                assert_eq!(backend.find_incoming_relationships(id_b).await.unwrap().len(), 1);
                assert_eq!(backend.count_relationships().await.unwrap(), 1);
            }

            #[tokio::test]
            async fn test_graph_clear() {
                let backend = make_backend().await;

                let file = FileInfo {
                    path: "src/to_delete.rs".to_string(),
                    hash: "clr000".to_string(),
                    language: Some("Rust".to_string()),
                    last_modified: None,
                };
                backend.insert_file(&file).await.unwrap();
                assert!(backend.count_files().await.unwrap() > 0);

                backend.clear().await.unwrap();

                assert_eq!(backend.count_files().await.unwrap(), 0);
                assert_eq!(backend.count_symbols().await.unwrap(), 0);
                assert_eq!(backend.count_symbol_refs().await.unwrap(), 0);
                assert_eq!(backend.count_relationships().await.unwrap(), 0);
            }

            // ═══════════════════════════════════════════════════
            // StorageBackend trait tests
            // ═══════════════════════════════════════════════════

            #[tokio::test]
            async fn test_health_check() {
                let backend = make_backend().await;
                backend.health_check().await.unwrap();
            }

            #[tokio::test]
            async fn test_backend_type() {
                let backend = make_backend().await;
                let type_name = backend.backend_type();
                assert!(!type_name.is_empty());
            }

            #[tokio::test]
            async fn test_close() {
                let backend = make_backend().await;
                backend.close().await.unwrap();
            }

            // ═══════════════════════════════════════════════════
            // Edge cases
            // ═══════════════════════════════════════════════════

            #[tokio::test]
            async fn test_load_nonexistent_session() {
                let backend = make_backend().await;
                let fake_id = crate::session::SessionId::new();
                let result = backend.load_session(&fake_id).await.unwrap();
                assert!(result.is_none());
            }

            #[tokio::test]
            async fn test_search_empty_query() {
                let backend = make_backend().await;
                let session = Session::new();
                backend.create_session(&session).await.unwrap();
                backend.save_message(&session.id, &Message::user("hello")).await.unwrap();

                let results = backend.search_messages("").await.unwrap();
                let _ = results; // just verify no panic
            }

            #[tokio::test]
            async fn test_delete_nonexistent_session() {
                let backend = make_backend().await;
                let fake_id = crate::session::SessionId::new();
                let result = backend.delete_session(&fake_id).await;
                let _ = result; // may succeed or fail — just verify no panic
            }
        }
    };
}

// ═══════════════════════════════════════════════════
// Test suite instantiations
// ═══════════════════════════════════════════════════

// InMemory backend (fully async, no setup needed)
backend_test_suite!(in_memory_backend, super::super::InMemoryBackend, {
    let backend = super::super::InMemoryBackend::new();
    backend.migrate().await.unwrap();
    backend
});

// SQLite backend (needs migrate)
backend_test_suite!(sqlite_backend, super::super::SqliteBackend, {
    let backend = super::super::SqliteBackend::in_memory().unwrap();
    backend.migrate().await.unwrap();
    backend
});
