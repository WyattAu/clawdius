//! Multi-language research synthesis and knowledge management.
//!
//! This module provides cross-lingual knowledge integration capabilities,
//! including concept graphs, automatic translation, and intelligent research synthesis.
//!
//! # Features
//!
//! - **Knowledge graphs**: Build and query concept relationship graphs
//! - **Multi-language support**: Work with research in multiple languages
//! - **Automatic translation**: Translate research content between languages
//! - **Research synthesis**: Combine multiple sources into coherent summaries
//! - **Concept extraction**: Extract and link concepts from text
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use clawdius_core::knowledge::{KnowledgeGraph, ResearchSynthesizer, ResearchQuery};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! // Create knowledge graph
//! let mut graph = KnowledgeGraph::new();
//!
//! // Add concepts and relationships
//! graph.add_concept("machine learning", "en")?;
//! graph.add_concept("apprentissage automatique", "fr")?;
//! graph.link_concepts("machine learning", "apprentissage automatique", "translation")?;
//!
//! // Query the graph
//! let related = graph.find_related("machine learning", 2)?;
//! for concept in related {
//!     println!("Related: {} ({})", concept.term, concept.language);
//! }
//!
//! // Synthesize research
//! let synthesizer = ResearchSynthesizer::new();
//! let query = ResearchQuery {
//!     topic: "transformer architectures".to_string(),
//!     languages: vec!["en".to_string(), "fr".to_string()],
//!     max_sources: 10,
//! };
//!
//! let result = synthesizer.synthesize(&query)?;
//! println!("Summary: {}", result.summary);
//! println!("Sources: {}", result.sources.len());
//! # Ok(())
//! # }
//! ```
//!
//! # Knowledge Graphs
//!
//! Build and query concept relationship graphs:
//!
//! ```rust,no_run
//! use clawdius_core::knowledge::{KnowledgeGraph, ConceptNode, ConceptEdge};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let mut graph = KnowledgeGraph::new();
//!
//! // Add concepts in different languages
//! graph.add_concept("neural network", "en")?;
//! graph.add_concept("réseau neuronal", "fr")?;
//! graph.add_concept("神经网络", "zh")?;
//!
//! // Create relationships
//! graph.link_concepts("neural network", "deep learning", "related_to")?;
//! graph.link_concepts("neural network", "réseau neuronal", "translation")?;
//!
//! // Find paths between concepts
//! let path = graph.find_path("neural network", "deep learning")?;
//! println!("Path length: {}", path.len());
//! # Ok(())
//! # }
//! ```
//!
//! # Language Detection and Translation
//!
//! Automatically detect and translate content:
//!
//! ```rust,no_run
//! use clawdius_core::knowledge::{Translator, detect_language};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! // Detect language
//! let text = "Bonjour, comment allez-vous?";
//! let lang = detect_language(text)?;
//! println!("Detected language: {}", lang);
//!
//! // Translate
//! let translator = Translator::new();
//! let translated = translator.translate(text, "en")?;
//! println!("Translated: {}", translated);
//! # Ok(())
//! # }
//! ```
//!
//! # Research Synthesis
//!
//! Combine multiple research sources:
//!
//! ```rust,no_run
//! use clawdius_core::knowledge::{ResearchSynthesizer, ResearchQuery};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let synthesizer = ResearchSynthesizer::new();
//!
//! let query = ResearchQuery {
//!     topic: "quantum computing algorithms".to_string(),
//!     languages: vec!["en".to_string()],
//!     max_sources: 5,
//! };
//!
//! let result = synthesizer.synthesize(&query)?;
//!
//! println!("Summary:\n{}", result.summary);
//! println!("\nKey findings:");
//! for finding in result.key_findings {
//!     println!("  - {}", finding);
//! }
//! println!("\nSources: {}", result.sources.len());
//! # Ok(())
//! # }
//! ```
//!
//! # Concept Extraction
//!
//! Extract concepts from text:
//!
//! ```rust,no_run
//! use clawdius_core::knowledge::{KnowledgeGraph, Language};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let mut graph = KnowledgeGraph::new();
//!
//! let text = "Machine learning uses neural networks for pattern recognition.";
//! let concepts = graph.extract_concepts(text, Language::English)?;
//!
//! for concept in concepts {
//!     println!("Concept: {} (importance: {:.2})", concept.term, concept.importance);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Supported Languages
//!
//! - English (`en`)
//! - French (`fr`)
//! - Spanish (`es`)
//! - German (`de`)
//! - Chinese (`zh`)
//! - Japanese (`ja`)
//! - Russian (`ru`)
//!
//! # Configuration
//!
//! Knowledge synthesis can be configured for different use cases:
//!
//! ```rust,no_run
//! use clawdius_core::knowledge::ResearchQuery;
//!
//! let query = ResearchQuery {
//!     topic: "renewable energy".to_string(),
//!     languages: vec!["en".to_string(), "de".to_string()],
//!     max_sources: 20,
//! };
//! ```

pub mod concepts;
pub mod graph;
pub mod synthesizer;
pub mod translator;

pub use concepts::{ConceptEdge, ConceptNode, Language, RelationshipType};
pub use graph::KnowledgeGraph;
pub use synthesizer::{ResearchQuery, ResearchSynthesizer, SynthesizedResult};
pub use translator::{detect_language, Translator};
