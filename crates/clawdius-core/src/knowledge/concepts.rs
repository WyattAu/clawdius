//! Concept types for knowledge graph

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

impl Default for Language {
    fn default() -> Self {
        Language::EN
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Language::EN => write!(f, "en"),
            Language::ZH => write!(f, "zh"),
            Language::RU => write!(f, "ru"),
            Language::DE => write!(f, "de"),
            Language::FR => write!(f, "fr"),
            Language::JP => write!(f, "jp"),
            Language::KO => write!(f, "ko"),
            Language::ES => write!(f, "es"),
            Language::IT => write!(f, "it"),
            Language::PT => write!(f, "pt"),
            Language::NL => write!(f, "nl"),
            Language::PL => write!(f, "pl"),
            Language::CS => write!(f, "cs"),
            Language::AR => write!(f, "ar"),
            Language::FA => write!(f, "fa"),
            Language::TR => write!(f, "tr"),
        }
    }
}

impl std::str::FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "en" | "english" => Ok(Language::EN),
            "zh" | "chinese" | "中文" => Ok(Language::ZH),
            "ru" | "russian" | "русский" => Ok(Language::RU),
            "de" | "german" | "deutsch" => Ok(Language::DE),
            "fr" | "french" | "français" => Ok(Language::FR),
            "jp" | "ja" | "japanese" | "日本語" => Ok(Language::JP),
            "ko" | "korean" | "한국어" => Ok(Language::KO),
            "es" | "spanish" | "español" => Ok(Language::ES),
            "it" | "italian" | "italiano" => Ok(Language::IT),
            "pt" | "portuguese" | "português" => Ok(Language::PT),
            "nl" | "dutch" | "nederlands" => Ok(Language::NL),
            "pl" | "polish" | "polski" => Ok(Language::PL),
            "cs" | "czech" | "čeština" => Ok(Language::CS),
            "ar" | "arabic" | "العربية" => Ok(Language::AR),
            "fa" | "farsi" | "persian" | "فارسی" => Ok(Language::FA),
            "tr" | "turkish" | "türkçe" => Ok(Language::TR),
            _ => Err(format!("Unknown language: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    SameAs,
    BroaderThan,
    NarrowerThan,
    RelatedTo,
    PartOf,
    HasPart,
}

impl fmt::Display for RelationshipType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelationshipType::SameAs => write!(f, "same_as"),
            RelationshipType::BroaderThan => write!(f, "broader_than"),
            RelationshipType::NarrowerThan => write!(f, "narrower_than"),
            RelationshipType::RelatedTo => write!(f, "related_to"),
            RelationshipType::PartOf => write!(f, "part_of"),
            RelationshipType::HasPart => write!(f, "has_part"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptNode {
    pub id: String,
    pub name: String,
    pub language: Language,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub definition: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default)]
    pub source: Option<String>,
}

fn default_confidence() -> f32 {
    1.0
}

impl ConceptNode {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        language: Language,
        definition: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            language,
            aliases: Vec::new(),
            definition: definition.into(),
            confidence: 1.0,
            source: None,
        }
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.name.to_lowercase().contains(&query_lower)
            || self.definition.to_lowercase().contains(&query_lower)
            || self
                .aliases
                .iter()
                .any(|a| a.to_lowercase().contains(&query_lower))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptEdge {
    pub from: String,
    pub to: String,
    pub relationship: RelationshipType,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

impl ConceptEdge {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        relationship: RelationshipType,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            relationship,
            confidence: 1.0,
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_language_from_str() {
        assert_eq!(Language::from_str("en").unwrap(), Language::EN);
        assert_eq!(Language::from_str("zh").unwrap(), Language::ZH);
        assert_eq!(Language::from_str("japanese").unwrap(), Language::JP);
        assert!(Language::from_str("unknown").is_err());
    }

    #[test]
    fn test_concept_node_creation() {
        let node = ConceptNode::new("ml_001", "Machine Learning", Language::EN, "A subset of AI")
            .with_alias("ML")
            .with_confidence(0.9);

        assert_eq!(node.id, "ml_001");
        assert_eq!(node.language, Language::EN);
        assert!(node.aliases.contains(&"ML".to_string()));
        assert_eq!(node.confidence, 0.9);
    }

    #[test]
    fn test_concept_matching() {
        let node = ConceptNode::new(
            "ai_001",
            "Artificial Intelligence",
            Language::EN,
            "Intelligence demonstrated by machines",
        )
        .with_alias("AI");

        assert!(node.matches_query("artificial"));
        assert!(node.matches_query("AI"));
        assert!(node.matches_query("machines"));
        assert!(!node.matches_query("cooking"));
    }

    #[test]
    fn test_relationship_type_display() {
        assert_eq!(RelationshipType::SameAs.to_string(), "same_as");
        assert_eq!(RelationshipType::BroaderThan.to_string(), "broader_than");
    }
}
