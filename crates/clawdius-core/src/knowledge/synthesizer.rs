//! Research synthesizer for cross-lingual knowledge integration

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::concepts::{ConceptEdge, ConceptNode, Language, RelationshipType};
use super::graph::KnowledgeGraph;
use super::translator::Translator;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchQuery {
    pub terms: Vec<String>,
    pub languages: Vec<Language>,
    pub max_results: usize,
}

impl ResearchQuery {
    #[must_use]
    pub fn new(terms: Vec<String>, languages: Vec<Language>) -> Self {
        Self {
            terms,
            languages,
            max_results: 10,
        }
    }

    #[must_use]
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    pub fn from_single_term(term: impl Into<String>, languages: Vec<Language>) -> Self {
        Self::new(vec![term.into()], languages)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesizedResult {
    pub concepts: Vec<ConceptNode>,
    pub relationships: Vec<ConceptEdge>,
    pub confidence: f32,
    pub sources: Vec<String>,
}

impl SynthesizedResult {
    #[must_use]
    pub fn new() -> Self {
        Self {
            concepts: Vec::new(),
            relationships: Vec::new(),
            confidence: 0.0,
            sources: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_concepts(mut self, concepts: Vec<ConceptNode>) -> Self {
        self.concepts = concepts;
        self
    }

    #[must_use]
    pub fn with_relationships(mut self, relationships: Vec<ConceptEdge>) -> Self {
        self.relationships = relationships;
        self
    }

    #[must_use]
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    #[must_use]
    pub fn with_sources(mut self, sources: Vec<String>) -> Self {
        self.sources = sources;
        self
    }

    pub fn calculate_confidence(&mut self) {
        if self.concepts.is_empty() {
            self.confidence = 0.0;
            return;
        }

        let avg_concept_confidence: f32 =
            self.concepts.iter().map(|c| c.confidence).sum::<f32>() / self.concepts.len() as f32;

        let avg_edge_confidence: f32 = if self.relationships.is_empty() {
            1.0
        } else {
            self.relationships.iter().map(|e| e.confidence).sum::<f32>()
                / self.relationships.len() as f32
        };

        let language_coverage = self.languages_covered().len() as f32 / 16.0;

        self.confidence =
            (avg_concept_confidence * 0.4 + avg_edge_confidence * 0.3 + language_coverage * 0.3)
                .min(1.0);
    }

    #[must_use]
    pub fn languages_covered(&self) -> Vec<Language> {
        let langs: HashSet<Language> = self.concepts.iter().map(|c| c.language).collect();
        let mut result: Vec<Language> = langs.into_iter().collect();
        result.sort_by_key(|l| format!("{l:?}"));
        result
    }

    #[must_use]
    pub fn concepts_by_language(&self, lang: Language) -> Vec<&ConceptNode> {
        self.concepts
            .iter()
            .filter(|c| c.language == lang)
            .collect()
    }
}

impl Default for SynthesizedResult {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ResearchSynthesizer {
    graph: KnowledgeGraph,
    translator: Translator,
}

impl ResearchSynthesizer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: KnowledgeGraph::new(),
            translator: Translator::new(),
        }
    }

    #[must_use]
    pub fn with_graph(graph: KnowledgeGraph) -> Self {
        Self {
            graph,
            translator: Translator::new(),
        }
    }

    pub async fn search_multilingual(&mut self, query: ResearchQuery) -> Result<SynthesizedResult> {
        let mut all_concepts: Vec<ConceptNode> = Vec::new();
        let mut all_relationships: Vec<ConceptEdge> = Vec::new();
        let mut sources: HashSet<String> = HashSet::new();

        for term in &query.terms {
            for lang in &query.languages {
                let translated = if *lang == Language::EN {
                    term.clone()
                } else {
                    self.translator
                        .translate(term, Language::EN, *lang)
                        .await
                        .unwrap_or_else(|_| term.clone())
                };

                let results = self.graph.search(&translated, Some(*lang));

                for concept in results {
                    all_concepts.push(concept.clone());

                    if let Some(ref src) = concept.source {
                        sources.insert(src.clone());
                    }

                    let equivalents = self.graph.find_equivalent(&concept.id);
                    for equiv in equivalents {
                        if !all_concepts.iter().any(|c| c.id == equiv.id) {
                            all_concepts.push(equiv.clone());

                            all_relationships.push(
                                ConceptEdge::new(
                                    concept.id.clone(),
                                    equiv.id.clone(),
                                    RelationshipType::SameAs,
                                )
                                .with_confidence(0.9),
                            );
                        }
                    }
                }
            }
        }

        all_concepts.truncate(query.max_results);
        all_relationships.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut result = SynthesizedResult::new()
            .with_concepts(all_concepts)
            .with_relationships(all_relationships)
            .with_sources(sources.into_iter().collect());

        result.calculate_confidence();

        Ok(result)
    }

    pub fn add_research(&mut self, source: &str, concepts: Vec<ConceptNode>) {
        let concept_ids: Vec<String> = concepts
            .iter()
            .map(|c| {
                let mut concept = c.clone();
                concept.source = Some(source.to_string());
                self.graph.add_concept(concept)
            })
            .collect();

        for i in 0..concept_ids.len() {
            for j in (i + 1)..concept_ids.len() {
                self.graph.add_relationship(
                    ConceptEdge::new(
                        &concept_ids[i],
                        &concept_ids[j],
                        RelationshipType::RelatedTo,
                    )
                    .with_confidence(0.5),
                );
            }
        }
    }

    pub fn add_cross_lingual_link(&mut self, from_id: &str, to_id: &str, confidence: f32) {
        self.graph.add_relationship(
            ConceptEdge::new(from_id, to_id, RelationshipType::SameAs).with_confidence(confidence),
        );
    }

    pub fn export_graph(&self) -> Result<String> {
        self.graph.export_json()
    }

    pub fn import_graph(&mut self, json: &str) -> Result<()> {
        self.graph = KnowledgeGraph::import_json(json)?;
        Ok(())
    }

    #[must_use]
    pub fn graph(&self) -> &KnowledgeGraph {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut KnowledgeGraph {
        &mut self.graph
    }

    #[must_use]
    pub fn translator(&self) -> &Translator {
        &self.translator
    }

    pub fn translator_mut(&mut self) -> &mut Translator {
        &mut self.translator
    }
}

impl Default for ResearchSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_synthesizer_creation() {
        let synthesizer = ResearchSynthesizer::new();
        assert_eq!(synthesizer.graph().count_nodes(), 0);
    }

    #[tokio::test]
    async fn test_add_research() {
        let mut synthesizer = ResearchSynthesizer::new();

        let concepts = vec![
            ConceptNode::new("ml_en", "Machine Learning", Language::EN, "AI subset"),
            ConceptNode::new("ml_zh", "机器学习", Language::ZH, "人工智能分支"),
        ];

        synthesizer.add_research("test_source", concepts);

        assert_eq!(synthesizer.graph().count_nodes(), 2);
        assert!(synthesizer.graph().count_edges() > 0);
    }

    #[tokio::test]
    async fn test_multilingual_search() {
        let mut synthesizer = ResearchSynthesizer::new();

        let mut concept1 = ConceptNode::new("ml_en", "Machine Learning", Language::EN, "AI subset");
        concept1.source = Some("source1".to_string());

        let mut concept2 = ConceptNode::new("ml_zh", "机器学习", Language::ZH, "人工智能分支");
        concept2.source = Some("source2".to_string());

        synthesizer.graph_mut().add_concept(concept1);
        synthesizer.graph_mut().add_concept(concept2);
        synthesizer.add_cross_lingual_link("ml_en", "ml_zh", 0.9);

        let query =
            ResearchQuery::from_single_term("Machine Learning", vec![Language::EN, Language::ZH])
                .with_max_results(10);

        let result = synthesizer.search_multilingual(query).await.unwrap();

        assert!(!result.concepts.is_empty());
        assert!(!result.relationships.is_empty());
        assert!(result.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_export_import_graph() {
        let mut synthesizer = ResearchSynthesizer::new();

        synthesizer.add_research(
            "source",
            vec![ConceptNode::new(
                "test_1",
                "Test",
                Language::EN,
                "Definition",
            )],
        );

        let json = synthesizer.export_graph().unwrap();

        let mut new_synthesizer = ResearchSynthesizer::new();
        new_synthesizer.import_graph(&json).unwrap();

        assert_eq!(new_synthesizer.graph().count_nodes(), 1);
    }

    #[test]
    fn test_synthesized_result_languages() {
        let result = SynthesizedResult::new().with_concepts(vec![
            ConceptNode::new("1", "A", Language::EN, "Def"),
            ConceptNode::new("2", "B", Language::ZH, "定义"),
            ConceptNode::new("3", "C", Language::EN, "Def2"),
        ]);

        let langs = result.languages_covered();
        assert_eq!(langs.len(), 2);
        assert!(langs.contains(&Language::EN));
        assert!(langs.contains(&Language::ZH));
    }

    #[test]
    fn test_research_query_builder() {
        let query = ResearchQuery::from_single_term("AI", vec![Language::EN, Language::ZH])
            .with_max_results(20);

        assert_eq!(query.terms, vec!["AI"]);
        assert_eq!(query.languages, vec![Language::EN, Language::ZH]);
        assert_eq!(query.max_results, 20);
    }
}
