//! Compliance Matrix Generation Module
//!
//! Generates compliance matrices for safety-critical and regulated domains.
//! Supports ISO/IEEE/IEC/NIST/DO-178C/EN standards.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Standard {
    Iso12207,
    Iso15288,
    Ieee1016,
    Ieee829,
    Iec61508,
    Iso26262,
    Do178c,
    En50128,
    Iec62304,
    Iec60880,
    NistSp800_53,
    NistSp800_145,
    Iec62443,
    Fips140_2,
    Fips140_3,
    Iso27001,
    Iso27034,
    Gdpr,
    Ccpa,
}

impl Standard {
    pub fn full_name(&self) -> &'static str {
        match self {
            Self::Iso12207 => "ISO/IEC 12207 - Software Life Cycle Processes",
            Self::Iso15288 => "ISO/IEC 15288 - System Life Cycle Processes",
            Self::Ieee1016 => "IEEE 1016 - Software Design Descriptions",
            Self::Ieee829 => "IEEE 829 - Software Test Documentation",
            Self::Iec61508 => "IEC 61508 - Functional Safety (Base)",
            Self::Iso26262 => "ISO 26262 - Road Vehicles Functional Safety",
            Self::Do178c => "DO-178C - Airborne Systems Certification",
            Self::En50128 => "EN 50128 - Railway Control Software",
            Self::Iec62304 => "IEC 62304 - Medical Device Software",
            Self::Iec60880 => "IEC 60880 - Nuclear Power Plant I&C",
            Self::NistSp800_53 => "NIST SP 800-53 - Security & Privacy Controls",
            Self::NistSp800_145 => "NIST SP 800-145 - Cloud Computing Definition",
            Self::Iec62443 => "IEC 62443 - Industrial Network Security",
            Self::Fips140_2 => "FIPS 140-2 - Cryptographic Modules",
            Self::Fips140_3 => "FIPS 140-3 - Cryptographic Modules",
            Self::Iso27001 => "ISO/IEC 27001 - Information Security Management",
            Self::Iso27034 => "ISO/IEC 27034 - Application Security",
            Self::Gdpr => "GDPR - EU Data Protection",
            Self::Ccpa => "CCPA - California Privacy",
        }
    }

    pub fn domain(&self) -> StandardDomain {
        match self {
            Self::Iso12207 | Self::Iso15288 => StandardDomain::Lifecycle,
            Self::Ieee1016 | Self::Ieee829 => StandardDomain::Documentation,
            Self::Iec61508
            | Self::Iso26262
            | Self::Do178c
            | Self::En50128
            | Self::Iec62304
            | Self::Iec60880 => StandardDomain::SafetyCritical,
            Self::NistSp800_53
            | Self::NistSp800_145
            | Self::Iec62443
            | Self::Fips140_2
            | Self::Fips140_3 => StandardDomain::Security,
            Self::Iso27001 | Self::Iso27034 => StandardDomain::InformationSecurity,
            Self::Gdpr | Self::Ccpa => StandardDomain::Privacy,
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            Self::Iec61508
            | Self::Iso26262
            | Self::Do178c
            | Self::En50128
            | Self::Iec62304
            | Self::Iec60880 => 100,
            Self::Fips140_2 | Self::Fips140_3 => 90,
            Self::NistSp800_53 | Self::Iec62443 => 80,
            Self::Gdpr | Self::Ccpa => 70,
            Self::Iso27001 | Self::Iso27034 => 60,
            Self::Iso12207 | Self::Iso15288 | Self::Ieee1016 | Self::Ieee829 => 50,
            Self::NistSp800_145 => 40,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "iso12207" | "iso/iec12207" => Some(Self::Iso12207),
            "iso15288" | "iso/iec15288" => Some(Self::Iso15288),
            "ieee1016" => Some(Self::Ieee1016),
            "ieee829" => Some(Self::Ieee829),
            "iec61508" => Some(Self::Iec61508),
            "iso26262" => Some(Self::Iso26262),
            "do178c" | "do-178c" => Some(Self::Do178c),
            "en50128" => Some(Self::En50128),
            "iec62304" => Some(Self::Iec62304),
            "iec60880" => Some(Self::Iec60880),
            "nistsp800-53" | "nist800-53" | "nist80053" => Some(Self::NistSp800_53),
            "nistsp800-145" | "nist800-145" => Some(Self::NistSp800_145),
            "iec62443" => Some(Self::Iec62443),
            "fips140-2" | "fips1402" => Some(Self::Fips140_2),
            "fips140-3" | "fips1403" => Some(Self::Fips140_3),
            "iso27001" => Some(Self::Iso27001),
            "iso27034" => Some(Self::Iso27034),
            "gdpr" => Some(Self::Gdpr),
            "ccpa" => Some(Self::Ccpa),
            _ => None,
        }
    }
}

impl std::fmt::Display for Standard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StandardDomain {
    Lifecycle,
    Documentation,
    SafetyCritical,
    Security,
    InformationSecurity,
    Privacy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub standard: Standard,
    pub clause: String,
    pub title: String,
    pub description: String,
    pub level: ComplianceLevel,
    pub evidence_type: EvidenceType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum ComplianceLevel {
    QM,
    SIL_A,
    SIL_B,
    SIL_C,
    SIL_D,
    ASIL_A,
    ASIL_B,
    ASIL_C,
    ASIL_D,
    DAL_A,
    DAL_B,
    DAL_C,
    DAL_D,
    DAL_E,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceType {
    Document,
    TestResults,
    CodeReview,
    FormalProof,
    AuditReport,
    TraceabilityMatrix,
    Inspection,
}

impl EvidenceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Document => "Document",
            Self::TestResults => "Test Results",
            Self::CodeReview => "Code Review",
            Self::FormalProof => "Formal Proof",
            Self::AuditReport => "Audit Report",
            Self::TraceabilityMatrix => "Traceability Matrix",
            Self::Inspection => "Inspection",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceMatrix {
    pub id: String,
    pub project_name: String,
    pub standards: Vec<Standard>,
    pub requirements: Vec<RequirementMapping>,
    pub gaps: Vec<ComplianceGap>,
    pub compliance_percentage: f32,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementMapping {
    pub requirement: Requirement,
    pub artifact_path: Option<String>,
    pub evidence_path: Option<String>,
    pub status: ComplianceStatus,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceStatus {
    Compliant,
    Partial,
    NonCompliant,
    NotApplicable,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceGap {
    pub requirement: Requirement,
    pub description: String,
    pub recommendation: String,
    pub severity: GapSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapSeverity {
    Critical,
    Major,
    Minor,
    Observation,
}

pub struct ComplianceMatrixGenerator {
    standards_db: HashMap<Standard, Vec<Requirement>>,
}

impl ComplianceMatrixGenerator {
    pub fn new() -> Self {
        Self {
            standards_db: Self::build_standards_db(),
        }
    }

    fn build_standards_db() -> HashMap<Standard, Vec<Requirement>> {
        let mut db = HashMap::new();

        db.insert(
            Standard::Iso26262,
            vec![
                Requirement {
                    id: "ISO26262-6:7.4.1".into(),
                    standard: Standard::Iso26262,
                    clause: "Part 6, Clause 7.4.1".into(),
                    title: "Software unit design".into(),
                    description: "Software units shall be designed according to the software detailed design".into(),
                    level: ComplianceLevel::ASIL_D,
                    evidence_type: EvidenceType::Document,
                },
                Requirement {
                    id: "ISO26262-6:7.4.10".into(),
                    standard: Standard::Iso26262,
                    clause: "Part 6, Clause 7.4.10".into(),
                    title: "Code review".into(),
                    description: "Code reviews shall be performed to verify compliance with coding guidelines".into(),
                    level: ComplianceLevel::ASIL_D,
                    evidence_type: EvidenceType::CodeReview,
                },
                Requirement {
                    id: "ISO26262-6:9.4.2".into(),
                    standard: Standard::Iso26262,
                    clause: "Part 6, Clause 9.4.2".into(),
                    title: "Unit testing".into(),
                    description: "Unit tests shall verify software unit requirements and structural coverage".into(),
                    level: ComplianceLevel::ASIL_D,
                    evidence_type: EvidenceType::TestResults,
                },
            ],
        );

        db.insert(
            Standard::Do178c,
            vec![
                Requirement {
                    id: "DO178C:6.3.1".into(),
                    standard: Standard::Do178c,
                    clause: "Section 6.3.1".into(),
                    title: "Source code standards".into(),
                    description: "Source code shall conform to defined coding standards".into(),
                    level: ComplianceLevel::DAL_A,
                    evidence_type: EvidenceType::CodeReview,
                },
                Requirement {
                    id: "DO178C:6.4.2".into(),
                    standard: Standard::Do178c,
                    clause: "Section 6.4.2".into(),
                    title: "MC/DC coverage".into(),
                    description:
                        "Modified Condition/Decision Coverage shall be achieved for Level A".into(),
                    level: ComplianceLevel::DAL_A,
                    evidence_type: EvidenceType::TestResults,
                },
            ],
        );

        db.insert(
            Standard::Iec62304,
            vec![
                Requirement {
                    id: "IEC62304:5.1.1".into(),
                    standard: Standard::Iec62304,
                    clause: "Clause 5.1.1".into(),
                    title: "Software development plan".into(),
                    description: "A software development plan shall be established".into(),
                    level: ComplianceLevel::General,
                    evidence_type: EvidenceType::Document,
                },
                Requirement {
                    id: "IEC62304:5.1.7".into(),
                    standard: Standard::Iec62304,
                    clause: "Clause 5.1.7".into(),
                    title: "Software traceability".into(),
                    description: "Traceability between requirements, design, code, and tests shall be maintained".into(),
                    level: ComplianceLevel::General,
                    evidence_type: EvidenceType::TraceabilityMatrix,
                },
            ],
        );

        db.insert(
            Standard::Ieee1016,
            vec![Requirement {
                id: "IEEE1016:5.1".into(),
                standard: Standard::Ieee1016,
                clause: "Clause 5.1".into(),
                title: "Design overview".into(),
                description: "Design description shall include purpose, scope, and stakeholder identification".into(),
                level: ComplianceLevel::General,
                evidence_type: EvidenceType::Document,
            }],
        );

        db.insert(
            Standard::NistSp800_53,
            vec![
                Requirement {
                    id: "NIST800-53:AC-2".into(),
                    standard: Standard::NistSp800_53,
                    clause: "Control AC-2".into(),
                    title: "Account management".into(),
                    description: "Manage information system accounts including creation, modification, and termination".into(),
                    level: ComplianceLevel::General,
                    evidence_type: EvidenceType::AuditReport,
                },
                Requirement {
                    id: "NIST800-53:SC-8".into(),
                    standard: Standard::NistSp800_53,
                    clause: "Control SC-8".into(),
                    title: "Transmission confidentiality".into(),
                    description: "Protect confidentiality of transmitted information".into(),
                    level: ComplianceLevel::General,
                    evidence_type: EvidenceType::TestResults,
                },
            ],
        );

        db.insert(
            Standard::Iec61508,
            vec![
                Requirement {
                    id: "IEC61508-3:7.4.2".into(),
                    standard: Standard::Iec61508,
                    clause: "Part 3, Clause 7.4.2".into(),
                    title: "Software architecture".into(),
                    description: "Software architecture shall be designed to achieve required safety integrity".into(),
                    level: ComplianceLevel::SIL_D,
                    evidence_type: EvidenceType::Document,
                },
                Requirement {
                    id: "IEC61508-3:7.9.2".into(),
                    standard: Standard::Iec61508,
                    clause: "Part 3, Clause 7.9.2".into(),
                    title: "Software verification".into(),
                    description: "Software shall be verified to ensure compliance with requirements".into(),
                    level: ComplianceLevel::SIL_D,
                    evidence_type: EvidenceType::TestResults,
                },
            ],
        );

        db.insert(
            Standard::En50128,
            vec![Requirement {
                id: "EN50128:5.3.2".into(),
                standard: Standard::En50128,
                clause: "Clause 5.3.2".into(),
                title: "Software requirements".into(),
                description: "Software requirements shall be specified and documented".into(),
                level: ComplianceLevel::SIL_D,
                evidence_type: EvidenceType::Document,
            }],
        );

        db.insert(
            Standard::Iec60880,
            vec![Requirement {
                id: "IEC60880:5.5".into(),
                standard: Standard::Iec60880,
                clause: "Clause 5.5".into(),
                title: "Software verification".into(),
                description: "Computer-based systems important to safety shall be verified".into(),
                level: ComplianceLevel::SIL_D,
                evidence_type: EvidenceType::FormalProof,
            }],
        );

        db.insert(
            Standard::Iso27001,
            vec![Requirement {
                id: "ISO27001:A.6.1.1".into(),
                standard: Standard::Iso27001,
                clause: "Annex A.6.1.1".into(),
                title: "Information security policy".into(),
                description: "An information security policy shall be established and maintained"
                    .into(),
                level: ComplianceLevel::General,
                evidence_type: EvidenceType::Document,
            }],
        );

        db.insert(
            Standard::Gdpr,
            vec![Requirement {
                id: "GDPR:Art.5".into(),
                standard: Standard::Gdpr,
                clause: "Article 5".into(),
                title: "Data processing principles".into(),
                description: "Personal data shall be processed lawfully, fairly and transparently"
                    .into(),
                level: ComplianceLevel::General,
                evidence_type: EvidenceType::AuditReport,
            }],
        );

        db
    }

    pub fn get_requirements(&self, standard: Standard) -> Vec<&Requirement> {
        self.standards_db
            .get(&standard)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn generate(
        &self,
        project_name: &str,
        standards: &[Standard],
        artifacts: &[Artifact],
    ) -> ComplianceMatrix {
        let mut requirements = Vec::new();
        let mut gaps = Vec::new();
        let mut compliant_count = 0;
        let mut total_count = 0;

        for standard in standards {
            if let Some(std_requirements) = self.standards_db.get(standard) {
                for req in std_requirements {
                    total_count += 1;

                    let mapping = self.map_requirement(req, artifacts);

                    if mapping.status == ComplianceStatus::Compliant {
                        compliant_count += 1;
                    } else if mapping.status == ComplianceStatus::NonCompliant {
                        gaps.push(ComplianceGap {
                            requirement: req.clone(),
                            description: format!("No evidence found for {}", req.id),
                            recommendation: format!(
                                "Provide {} evidence for {}",
                                req.evidence_type.as_str(),
                                req.id
                            ),
                            severity: GapSeverity::Major,
                        });
                    }

                    requirements.push(mapping);
                }
            }
        }

        let compliance_percentage = if total_count > 0 {
            (compliant_count as f32 / total_count as f32) * 100.0
        } else {
            0.0
        };

        ComplianceMatrix {
            id: format!("CM-{}", uuid::Uuid::new_v4()),
            project_name: project_name.into(),
            standards: standards.to_vec(),
            requirements,
            gaps,
            compliance_percentage,
            generated_at: generate_timestamp(),
        }
    }

    fn map_requirement(
        &self,
        requirement: &Requirement,
        artifacts: &[Artifact],
    ) -> RequirementMapping {
        let matching_artifact = artifacts.iter().find(|a| a.satisfies(requirement));

        match matching_artifact {
            Some(artifact) => RequirementMapping {
                requirement: requirement.clone(),
                artifact_path: Some(artifact.path.clone()),
                evidence_path: Some(artifact.evidence_path.clone()),
                status: ComplianceStatus::Compliant,
                notes: Some(format!("Satisfied by {}", artifact.name)),
            },
            None => RequirementMapping {
                requirement: requirement.clone(),
                artifact_path: None,
                evidence_path: None,
                status: ComplianceStatus::NonCompliant,
                notes: Some("No evidence provided".into()),
            },
        }
    }

    pub fn export_markdown(&self, matrix: &ComplianceMatrix) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Compliance Matrix: {}\n\n", matrix.project_name));
        md.push_str(&format!("**Generated:** {}\n\n", matrix.generated_at));
        md.push_str(&format!(
            "**Compliance:** {:.1}%\n\n",
            matrix.compliance_percentage
        ));

        md.push_str("## Standards Covered\n\n");
        for standard in &matrix.standards {
            md.push_str(&format!(
                "- {} (priority: {})\n",
                standard.full_name(),
                standard.priority()
            ));
        }

        md.push_str("\n## Requirements Mapping\n\n");
        md.push_str("| Requirement | Standard | Status | Evidence |\n");
        md.push_str("|-------------|----------|--------|----------|\n");

        for mapping in &matrix.requirements {
            let status = match mapping.status {
                ComplianceStatus::Compliant => "Compliant",
                ComplianceStatus::Partial => "Partial",
                ComplianceStatus::NonCompliant => "Non-Compliant",
                ComplianceStatus::NotApplicable => "N/A",
                ComplianceStatus::Pending => "Pending",
            };

            let evidence = mapping.evidence_path.as_deref().unwrap_or("-");

            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                mapping.requirement.id, mapping.requirement.standard, status, evidence
            ));
        }

        if !matrix.gaps.is_empty() {
            md.push_str("\n## Compliance Gaps\n\n");
            for gap in &matrix.gaps {
                md.push_str(&format!(
                    "### {} ({:?})\n\n{}\n\n**Recommendation:** {}\n\n",
                    gap.requirement.id, gap.severity, gap.description, gap.recommendation
                ));
            }
        }

        md
    }

    pub fn export_toml(&self, matrix: &ComplianceMatrix) -> String {
        toml::to_string_pretty(matrix).unwrap_or_else(|e| format!("# Error: {}", e))
    }
}

impl Default for ComplianceMatrixGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub path: String,
    pub evidence_path: String,
    pub artifact_type: ArtifactType,
    pub satisfies_requirements: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactType {
    SourceCode,
    TestCode,
    Documentation,
    Specification,
    Proof,
    Report,
}

impl Artifact {
    pub fn satisfies(&self, requirement: &Requirement) -> bool {
        self.satisfies_requirements.contains(&requirement.id)
    }
}

pub fn parse_standards(input: &str) -> Vec<Standard> {
    input
        .split(',')
        .filter_map(|s| Standard::from_str(s.trim()))
        .collect()
}

pub fn scan_for_artifacts(path: &std::path::Path) -> Vec<Artifact> {
    let mut artifacts = Vec::new();

    let docs_path = path.join("docs");
    if docs_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&docs_path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.extension().map(|e| e == "md").unwrap_or(false) {
                    let name = entry_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    artifacts.push(Artifact {
                        name: name.clone(),
                        path: entry_path.to_string_lossy().to_string(),
                        evidence_path: entry_path.to_string_lossy().to_string(),
                        artifact_type: ArtifactType::Documentation,
                        satisfies_requirements: infer_satisfied_requirements(&name),
                    });
                }
            }
        }
    }

    let tests_path = path.join("tests");
    if tests_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&tests_path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.extension().map(|e| e == "rs").unwrap_or(false) {
                    let name = entry_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    artifacts.push(Artifact {
                        name: name.clone(),
                        path: entry_path.to_string_lossy().to_string(),
                        evidence_path: entry_path.to_string_lossy().to_string(),
                        artifact_type: ArtifactType::TestCode,
                        satisfies_requirements: infer_satisfied_requirements(&name),
                    });
                }
            }
        }
    }

    artifacts
}

fn infer_satisfied_requirements(filename: &str) -> Vec<String> {
    let mut requirements = Vec::new();
    let lower = filename.to_lowercase();

    if lower.contains("review") || lower.contains("code_review") {
        requirements.push("ISO26262-6:7.4.10".into());
        requirements.push("DO178C:6.3.1".into());
    }
    if lower.contains("test") || lower.contains("unit") {
        requirements.push("ISO26262-6:9.4.2".into());
        requirements.push("DO178C:6.4.2".into());
    }
    if lower.contains("design") || lower.contains("architecture") {
        requirements.push("ISO26262-6:7.4.1".into());
        requirements.push("IEC61508-3:7.4.2".into());
    }
    if lower.contains("plan") || lower.contains("sdp") {
        requirements.push("IEC62304:5.1.1".into());
    }
    if lower.contains("traceability") || lower.contains("trace") {
        requirements.push("IEC62304:5.1.7".into());
    }

    requirements
}

fn generate_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format_timestamp(duration.as_secs() as i64)
}

fn format_timestamp(secs: i64) -> String {
    let days = secs / 86400;
    let year = 1970 + days / 365;
    let month = ((days % 365) / 30) + 1;
    let day = (days % 30) + 1;
    let hour = (secs % 86400) / 3600;
    let minute = (secs % 3600) / 60;
    let second = secs % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_full_name() {
        assert!(Standard::Iso26262.full_name().contains("26262"));
        assert!(Standard::Do178c.full_name().contains("178"));
    }

    #[test]
    fn test_standard_priority() {
        assert!(Standard::Iec61508.priority() > Standard::Iso12207.priority());
        assert!(Standard::Iso26262.priority() > Standard::Ieee1016.priority());
    }

    #[test]
    fn test_compliance_matrix_generator_creation() {
        let generator = ComplianceMatrixGenerator::new();
        let requirements = generator.get_requirements(Standard::Iso26262);
        assert!(!requirements.is_empty());
    }

    #[test]
    fn test_compliance_matrix_generation() {
        let generator = ComplianceMatrixGenerator::new();
        let matrix =
            generator.generate("Test Project", &[Standard::Iso26262, Standard::Do178c], &[]);

        assert!(!matrix.requirements.is_empty());
        assert!(!matrix.standards.is_empty());
    }

    #[test]
    fn test_compliance_matrix_with_artifacts() {
        let generator = ComplianceMatrixGenerator::new();
        let artifacts = vec![Artifact {
            name: "Code Review Report".into(),
            path: "docs/reviews/code.md".into(),
            evidence_path: "docs/reviews/code.md".into(),
            artifact_type: ArtifactType::Report,
            satisfies_requirements: vec!["ISO26262-6:7.4.10".into()],
        }];

        let matrix = generator.generate("Test Project", &[Standard::Iso26262], &artifacts);

        let compliant = matrix
            .requirements
            .iter()
            .filter(|r| r.status == ComplianceStatus::Compliant)
            .count();
        assert!(compliant > 0);
    }

    #[test]
    fn test_markdown_export() {
        let generator = ComplianceMatrixGenerator::new();
        let matrix = generator.generate("Test Project", &[Standard::Iso26262], &[]);

        let md = generator.export_markdown(&matrix);
        assert!(md.contains("# Compliance Matrix"));
        assert!(md.contains("Test Project"));
    }

    #[test]
    fn test_requirement_serialization() {
        let req = Requirement {
            id: "TEST-001".into(),
            standard: Standard::Iso26262,
            clause: "6.7.4".into(),
            title: "Test".into(),
            description: "Test requirement".into(),
            level: ComplianceLevel::ASIL_D,
            evidence_type: EvidenceType::TestResults,
        };

        let json = serde_json::to_string(&req).unwrap();
        let deserialized: Requirement = serde_json::from_str(&json).unwrap();
        assert_eq!(req.id, deserialized.id);
    }

    #[test]
    fn test_parse_standards() {
        let standards = parse_standards("iso26262,do178c,iec62304");
        assert_eq!(standards.len(), 3);
        assert!(standards.contains(&Standard::Iso26262));
        assert!(standards.contains(&Standard::Do178c));
        assert!(standards.contains(&Standard::Iec62304));
    }

    #[test]
    fn test_standard_from_str() {
        assert_eq!(Standard::from_str("iso26262"), Some(Standard::Iso26262));
        assert_eq!(Standard::from_str("do-178c"), Some(Standard::Do178c));
        assert_eq!(Standard::from_str("invalid"), None);
    }
}
