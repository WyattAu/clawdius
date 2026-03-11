//! Multi-Lingual Knowledge Integration Module
//!
//! Provides cross-lingual research synthesis and concept mapping
//! for the Graph-RAG knowledge layer.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    EN,
    ZH,
    RU,
    DE,
    FR,
    JP,
    KO,
    ES,
    IT,
    PT,
    NL,
    PL,
    CS,
    AR,
    FA,
    TR,
}

impl Language {
    pub fn name(&self) -> &'static str {
        match self {
            Self::EN => "English",
            Self::ZH => "Chinese",
            Self::RU => "Russian",
            Self::DE => "German",
            Self::FR => "French",
            Self::JP => "Japanese",
            Self::KO => "Korean",
            Self::ES => "Spanish",
            Self::IT => "Italian",
            Self::PT => "Portuguese",
            Self::NL => "Dutch",
            Self::PL => "Polish",
            Self::CS => "Czech",
            Self::AR => "Arabic",
            Self::FA => "Persian",
            Self::TR => "Turkish",
        }
    }

    pub fn native_name(&self) -> &'static str {
        match self {
            Self::EN => "English",
            Self::ZH => "中文",
            Self::RU => "Русский",
            Self::DE => "Deutsch",
            Self::FR => "Français",
            Self::JP => "日本語",
            Self::KO => "한국어",
            Self::ES => "Español",
            Self::IT => "Italiano",
            Self::PT => "Português",
            Self::NL => "Nederlands",
            Self::PL => "Polski",
            Self::CS => "Čeština",
            Self::AR => "العربية",
            Self::FA => "فارسی",
            Self::TR => "Türkçe",
        }
    }

    pub fn databases(&self) -> &[&'static str] {
        match self {
            Self::EN => &["arxiv.org", "ieeexplore.ieee.org", "dl.acm.org", "scholar.google.com"],
            Self::ZH => &["cnki.net", "wanfangdata.com.cn", "csdn.net", "arxiv.org.cn"],
            Self::RU => &["elibrary.ru", "cyberleninka.ru", "mathnet.ru"],
            Self::DE => &["springer.com", "tib.eu", "dnb.de"],
            Self::FR => &["hal.science", "cnrs.fr", "inria.fr"],
            Self::JP => &["jstage.jst.go.jp", "ci.nii.ac.jp", "jairo.nii.ac.jp"],
            Self::KO => &["dbpia.co.kr", "riss.kr", "kci.go.kr"],
            Self::ES => &["dialnet.unirioja.es", "redalyc.org", "scielo.org"],
            Self::IT => &["cnr.it"],
            Self::PT => &["scielo.org"],
            Self::NL => &["narcis.nl"],
            Self::PL => &["bazekon.icm.edu.pl"],
            Self::CS => &["dml.cz"],
            Self::AR => &["search.ebscohost.com"],
            Self::FA => &["magiran.com"],
            Self::TR => &["dergipark.gov.tr"],
        }
    }

    pub fn is_primary(&self) -> bool {
        matches!(self, Self::EN | Self::ZH | Self::RU | Self::DE | Self::JP)
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TqaLevel {
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Level4 = 4,
    Level5 = 5,
}

impl TqaLevel {
    pub fn min_confidence(&self) -> f32 {
        match self {
            Self::Level1 => 0.30,
            Self::Level2 => 0.50,
            Self::Level3 => 0.70,
            Self::Level4 => 0.85,
            Self::Level5 => 0.95,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Level1 => "Machine Translation - Initial screening only",
            Self::Level2 => "Back-Translation - Preliminary understanding",
            Self::Level3 => "Technical Review - Technical analysis",
            Self::Level4 => "Peer Validation - Critical technical decisions",
            Self::Level5 => "Expert Consensus - Safety-critical decisions",
        }
    }

    pub fn for_material(material_type: MaterialType) -> Self {
        match material_type {
            MaterialType::SafetyCriticalAlgorithm => Self::Level5,
            MaterialType::TheoremProof => Self::Level5,
            MaterialType::SecurityDefinition => Self::Level5,
            MaterialType::ArchitecturalDecision => Self::Level4,
            MaterialType::DomainConstraint => Self::Level4,
            MaterialType::GeneralResearch => Self::Level3,
            MaterialType::PreliminaryScreening => Self::Level2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialType {
    SafetyCriticalAlgorithm,
    TheoremProof,
    SecurityDefinition,
    ArchitecturalDecision,
    DomainConstraint,
    GeneralResearch,
    PreliminaryScreening,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    pub name: String,
    pub terms: HashMap<Language, String>,
    pub tqa_level: TqaLevel,
    pub confidence: f32,
    pub sources: Vec<Source>,
    pub related: Vec<String>,
    pub status: ConceptStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConceptStatus {
    Active,
    Deprecated,
    Conflicting,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub source_type: SourceType,
    pub language: Language,
    pub title: String,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    PeerReviewed,
    Standard,
    TechnicalBlog,
    OfficialDoc,
    AcademicDatabase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchFinding {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub original_language: Language,
    pub original_text: Option<String>,
    pub tqa_level: TqaLevel,
    pub confidence: f32,
    pub concepts: Vec<String>,
    pub sources: Vec<Source>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisRequest {
    pub query: String,
    pub languages: Vec<Language>,
    pub min_tqa_level: TqaLevel,
    pub max_results_per_language: usize,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisResult {
    pub query: String,
    pub findings: HashMap<Language, Vec<ResearchFinding>>,
    pub synthesis: String,
    pub concepts: Vec<Concept>,
    pub confidence: f32,
    pub tqa_level: TqaLevel,
}

pub struct KnowledgeGraph {
    concepts: HashMap<String, Concept>,
    language_index: HashMap<Language, Vec<String>>,
    term_index: HashMap<String, Vec<String>>,
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            concepts: HashMap::new(),
            language_index: HashMap::new(),
            term_index: HashMap::new(),
        }
    }

    pub fn load_from_markdown(path: &Path) -> Result<Self, KnowledgeError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| KnowledgeError::IoError(e.to_string()))?;

        let mut graph = Self::new();

        for line in content.lines() {
            if line.starts_with("| CONCEPT-") {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 7 {
                    let concept = Concept {
                        id: parts[1].trim().to_string(),
                        name: parts[2].trim().to_string(),
                        terms: HashMap::new(),
                        tqa_level: TqaLevel::Level3,
                        confidence: 0.8,
                        sources: Vec::new(),
                        related: Vec::new(),
                        status: ConceptStatus::Active,
                    };
                    graph.add_concept(concept);
                }
            }
        }

        Ok(graph)
    }

    pub fn add_concept(&mut self, concept: Concept) {
        for (lang, term) in &concept.terms {
            self.language_index.entry(*lang).or_default().push(concept.id.clone());
            self.term_index.entry(term.clone()).or_default().push(concept.id.clone());
        }

        self.term_index.entry(concept.name.clone()).or_default().push(concept.id.clone());

        self.concepts.insert(concept.id.clone(), concept);
    }

    pub fn get_concept(&self, id: &str) -> Option<&Concept> {
        self.concepts.get(id)
    }

    pub fn search(&self, term: &str) -> Vec<&Concept> {
        self.term_index
            .get(term)
            .map(|ids| ids.iter().filter_map(|id| self.concepts.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn by_language(&self, lang: Language) -> Vec<&Concept> {
        self.language_index
            .get(&lang)
            .map(|ids| ids.iter().filter_map(|id| self.concepts.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn export_jsonld(&self) -> serde_json::Value {
        let nodes: Vec<serde_json::Value> = self.concepts.values().map(|c| {
            serde_json::json!({
                "@id": c.id,
                "@type": "Concept",
                "name": c.name,
                "confidence": c.confidence,
                "tqa_level": c.tqa_level as i32,
                "terms": c.terms.iter().map(|(k, v)| {
                    serde_json::json!({
                        "language": k.to_string(),
                        "term": v
                    })
                }).collect::<Vec<_>>()
            })
        }).collect();

        serde_json::json!({
            "@context": {
                "@vocab": "https://clawdius.dev/knowledge/"
            },
            "@graph": nodes
        })
    }

    pub fn count(&self) -> usize {
        self.concepts.len()
    }
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ResearchSynthesizer {
    graph: KnowledgeGraph,
    llm_client: crate::llm::LlmClient,
}

impl ResearchSynthesizer {
    pub fn new() -> Self {
        Self {
            graph: KnowledgeGraph::new(),
            llm_client: crate::llm::LlmClient::new(),
        }
    }

    pub fn with_graph(graph: KnowledgeGraph) -> Self {
        Self {
            graph,
            llm_client: crate::llm::LlmClient::new(),
        }
    }

    pub fn load_graph(&mut self, path: &Path) -> Result<(), KnowledgeError> {
        self.graph = KnowledgeGraph::load_from_markdown(path)?;
        Ok(())
    }

    pub async fn synthesize(&self, request: SynthesisRequest) -> SynthesisResult {
        let mut findings: HashMap<Language, Vec<ResearchFinding>> = HashMap::new();
        let mut all_concepts: Vec<Concept> = Vec::new();
        let mut total_confidence = 0.0;
        let mut confidence_count = 0;

        for lang in &request.languages {
            let concepts = self.graph.by_language(*lang);
            let mut lang_findings = Vec::new();

            for concept in concepts.iter().take(request.max_results_per_language) {
                if concept.confidence >= request.min_tqa_level.min_confidence() {
                    let finding = ResearchFinding {
                        id: format!("FIND-{}", uuid::Uuid::new_v4()),
                        title: concept.name.clone(),
                        summary: format!("Concept: {} (TQA Level {})", 
                            concept.name, concept.tqa_level as i32),
                        original_language: *lang,
                        original_text: concept.terms.get(lang).cloned(),
                        tqa_level: concept.tqa_level,
                        confidence: concept.confidence,
                        concepts: vec![concept.id.clone()],
                        sources: concept.sources.clone(),
                        tags: Vec::new(),
                    };

                    total_confidence += concept.confidence;
                    confidence_count += 1;
                    lang_findings.push(finding);
                }
            }

            all_concepts.extend(concepts.into_iter().cloned());
            findings.insert(*lang, lang_findings);
        }

        let synthesis = self.generate_synthesis(&findings, &request.query);

        let avg_confidence = if confidence_count > 0 {
            total_confidence / confidence_count as f32
        } else {
            0.0
        };

        let tqa_level = self.determine_tqa_level(avg_confidence);

        SynthesisResult {
            query: request.query,
            findings,
            synthesis,
            concepts: all_concepts,
            confidence: avg_confidence,
            tqa_level,
        }
    }

    fn generate_synthesis(
        &self,
        findings: &HashMap<Language, Vec<ResearchFinding>>,
        query: &str,
    ) -> String {
        let total_findings: usize = findings.values().map(|v| v.len()).sum();
        let languages: Vec<_> = findings.keys().collect();

        format!(
            "Synthesis for '{}': Found {} relevant findings across {} languages ({}). \
             Cross-lingual research synthesis enables comprehensive coverage of SOTA research.",
            query,
            total_findings,
            languages.len(),
            languages.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ")
        )
    }

    fn determine_tqa_level(&self, confidence: f32) -> TqaLevel {
        if confidence >= TqaLevel::Level5.min_confidence() {
            TqaLevel::Level5
        } else if confidence >= TqaLevel::Level4.min_confidence() {
            TqaLevel::Level4
        } else if confidence >= TqaLevel::Level3.min_confidence() {
            TqaLevel::Level3
        } else if confidence >= TqaLevel::Level2.min_confidence() {
            TqaLevel::Level2
        } else {
            TqaLevel::Level1
        }
    }

    pub fn graph(&self) -> &KnowledgeGraph {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut KnowledgeGraph {
        &mut self.graph
    }
}

impl Default for ResearchSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum KnowledgeError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Translation error: {0}")]
    TranslationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_names() {
        assert_eq!(Language::EN.name(), "English");
        assert_eq!(Language::ZH.name(), "Chinese");
        assert_eq!(Language::JP.native_name(), "日本語");
    }

    #[test]
    fn test_language_databases() {
        let dbs = Language::EN.databases();
        assert!(!dbs.is_empty());
        assert!(dbs.contains(&"arxiv.org"));
    }

    #[test]
    fn test_tqa_levels() {
        assert!((TqaLevel::Level5.min_confidence() - 0.95).abs() < 0.001);
        assert!((TqaLevel::Level3.min_confidence() - 0.70).abs() < 0.001);
    }

    #[test]
    fn test_tqa_for_material() {
        assert_eq!(
            TqaLevel::for_material(MaterialType::SafetyCriticalAlgorithm),
            TqaLevel::Level5
        );
        assert_eq!(
            TqaLevel::for_material(MaterialType::GeneralResearch),
            TqaLevel::Level3
        );
    }

    #[test]
    fn test_concept_creation() {
        let mut terms = HashMap::new();
        terms.insert(Language::EN, "Finite State Machine".into());
        terms.insert(Language::ZH, "有限状态机".into());

        let concept = Concept {
            id: "CONCEPT-FSM-001".into(),
            name: "Finite State Machine".into(),
            terms,
            tqa_level: TqaLevel::Level5,
            confidence: 0.98,
            sources: Vec::new(),
            related: Vec::new(),
            status: ConceptStatus::Active,
        };

        assert_eq!(concept.id, "CONCEPT-FSM-001");
        assert_eq!(concept.tqa_level, TqaLevel::Level5);
    }

    #[test]
    fn test_knowledge_graph() {
        let mut graph = KnowledgeGraph::new();

        let mut terms = HashMap::new();
        terms.insert(Language::EN, "Test Concept".into());

        let concept = Concept {
            id: "TEST-001".into(),
            name: "Test Concept".into(),
            terms,
            tqa_level: TqaLevel::Level3,
            confidence: 0.8,
            sources: Vec::new(),
            related: Vec::new(),
            status: ConceptStatus::Active,
        };

        graph.add_concept(concept);

        assert_eq!(graph.count(), 1);
        assert!(graph.get_concept("TEST-001").is_some());
    }

    #[test]
    fn test_knowledge_graph_search() {
        let mut graph = KnowledgeGraph::new();

        let mut terms = HashMap::new();
        terms.insert(Language::EN, "Ring Buffer".into());
        terms.insert(Language::ZH, "环形缓冲区".into());

        let concept = Concept {
            id: "CONCEPT-RING-001".into(),
            name: "Circular Buffer".into(),
            terms,
            tqa_level: TqaLevel::Level5,
            confidence: 0.97,
            sources: Vec::new(),
            related: Vec::new(),
            status: ConceptStatus::Active,
        };

        graph.add_concept(concept);

        let results = graph.search("Ring Buffer");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Circular Buffer");
    }

    #[test]
    fn test_jsonld_export() {
        let mut graph = KnowledgeGraph::new();

        let concept = Concept {
            id: "TEST-002".into(),
            name: "Test".into(),
            terms: HashMap::new(),
            tqa_level: TqaLevel::Level3,
            confidence: 0.8,
            sources: Vec::new(),
            related: Vec::new(),
            status: ConceptStatus::Active,
        };

        graph.add_concept(concept);

        let json = graph.export_jsonld();
        assert!(json.is_object());
    }

    #[test]
    fn test_synthesis_request() {
        let request = SynthesisRequest {
            query: "finite state machine".into(),
            languages: vec![Language::EN, Language::ZH],
            min_tqa_level: TqaLevel::Level3,
            max_results_per_language: 10,
            domain: Some("control systems".into()),
        };

        assert_eq!(request.languages.len(), 2);
    }

    #[test]
    fn test_language_is_primary() {
        assert!(Language::EN.is_primary());
        assert!(Language::ZH.is_primary());
        assert!(!Language::NL.is_primary());
    }
}
