//! Knowledge graph implementation

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::concepts::{ConceptEdge, ConceptNode, Language, RelationshipType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    nodes: HashMap<String, ConceptNode>,
    edges: Vec<ConceptEdge>,
    #[serde(default)]
    language_index: HashMap<Language, HashSet<String>>,
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            language_index: HashMap::new(),
        }
    }

    pub fn add_concept(&mut self, concept: ConceptNode) -> String {
        let id = concept.id.clone();
        let lang = concept.language;

        self.nodes.insert(id.clone(), concept);
        self.language_index
            .entry(lang)
            .or_default()
            .insert(id.clone());

        id
    }

    pub fn add_relationship(&mut self, edge: ConceptEdge) {
        self.edges.push(edge);
    }

    pub fn find_by_language(&self, lang: Language) -> Vec<&ConceptNode> {
        self.language_index
            .get(&lang)
            .map(|ids| ids.iter().filter_map(|id| self.nodes.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn find_equivalent(&self, concept_id: &str) -> Vec<&ConceptNode> {
        let equivalents: HashSet<String> = self
            .edges
            .iter()
            .filter(|e| e.relationship == RelationshipType::SameAs)
            .filter(|e| e.from == concept_id || e.to == concept_id)
            .map(|e| if e.from == concept_id { &e.to } else { &e.from })
            .cloned()
            .collect();

        equivalents
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .collect()
    }

    pub fn search(&self, query: &str, lang: Option<Language>) -> Vec<&ConceptNode> {
        let candidates: Vec<&ConceptNode> = match lang {
            Some(l) => self.find_by_language(l),
            None => self.nodes.values().collect(),
        };

        candidates
            .into_iter()
            .filter(|node| node.matches_query(query))
            .collect()
    }

    pub fn get_concept(&self, id: &str) -> Option<&ConceptNode> {
        self.nodes.get(id)
    }

    pub fn get_concepts(&self) -> impl Iterator<Item = &ConceptNode> {
        self.nodes.values()
    }

    pub fn get_edges(&self) -> &[ConceptEdge] {
        &self.edges
    }

    pub fn find_related(&self, concept_id: &str) -> Vec<&ConceptEdge> {
        self.edges
            .iter()
            .filter(|e| e.from == concept_id || e.to == concept_id)
            .collect()
    }

    pub fn count_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn count_edges(&self) -> usize {
        self.edges.len()
    }

    pub fn languages(&self) -> Vec<Language> {
        self.language_index.keys().copied().collect()
    }

    pub fn export_json(&self) -> crate::Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| crate::Error::Serialization(e))
    }

    pub fn import_json(json: &str) -> crate::Result<Self> {
        serde_json::from_str(json).map_err(|e| crate::Error::Serialization(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_creation() {
        let graph = KnowledgeGraph::new();
        assert_eq!(graph.count_nodes(), 0);
        assert_eq!(graph.count_edges(), 0);
    }

    #[test]
    fn test_add_concept() {
        let mut graph = KnowledgeGraph::new();
        let concept = ConceptNode::new("test_1", "Test", Language::EN, "A test concept");

        graph.add_concept(concept);

        assert_eq!(graph.count_nodes(), 1);
        assert!(graph.get_concept("test_1").is_some());
    }

    #[test]
    fn test_find_by_language() {
        let mut graph = KnowledgeGraph::new();

        graph.add_concept(ConceptNode::new("en_1", "Hello", Language::EN, "Greeting"));
        graph.add_concept(ConceptNode::new("zh_1", "你好", Language::ZH, "问候"));
        graph.add_concept(ConceptNode::new("en_2", "World", Language::EN, "The world"));

        let en_concepts = graph.find_by_language(Language::EN);
        assert_eq!(en_concepts.len(), 2);

        let zh_concepts = graph.find_by_language(Language::ZH);
        assert_eq!(zh_concepts.len(), 1);
    }

    #[test]
    fn test_find_equivalent() {
        let mut graph = KnowledgeGraph::new();

        graph.add_concept(ConceptNode::new(
            "en_1",
            "Computer",
            Language::EN,
            "Computing device",
        ));
        graph.add_concept(ConceptNode::new("zh_1", "电脑", Language::ZH, "计算设备"));
        graph.add_concept(ConceptNode::new(
            "de_1",
            "Computer",
            Language::DE,
            "Rechenmaschine",
        ));

        graph.add_relationship(ConceptEdge::new("en_1", "zh_1", RelationshipType::SameAs));
        graph.add_relationship(ConceptEdge::new("en_1", "de_1", RelationshipType::SameAs));

        let equivalents = graph.find_equivalent("en_1");
        assert_eq!(equivalents.len(), 2);
    }

    #[test]
    fn test_search() {
        let mut graph = KnowledgeGraph::new();

        graph.add_concept(ConceptNode::new(
            "ml_1",
            "Machine Learning",
            Language::EN,
            "AI subset",
        ));
        graph.add_concept(ConceptNode::new(
            "ml_2",
            "机器学习",
            Language::ZH,
            "人工智能分支",
        ));
        graph.add_concept(ConceptNode::new(
            "dl_1",
            "Deep Learning",
            Language::EN,
            "Neural networks",
        ));

        let results = graph.search("learning", Some(Language::EN));
        assert_eq!(results.len(), 2);

        let results = graph.search("AI", None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_export_import() {
        let mut graph = KnowledgeGraph::new();
        graph.add_concept(ConceptNode::new("t1", "Test", Language::EN, "Definition"));

        let json = graph.export_json().unwrap();
        let imported = KnowledgeGraph::import_json(&json).unwrap();

        assert_eq!(imported.count_nodes(), 1);
        assert!(imported.get_concept("t1").is_some());
    }
}
