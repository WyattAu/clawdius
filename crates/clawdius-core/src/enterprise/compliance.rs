//! Compliance Framework Support
//!
//! Provides compliance templates and reporting for various regulatory frameworks.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported compliance frameworks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComplianceFramework {
    /// SOC 2 Type II
    Soc2Type2,
    /// ISO 27001
    Iso27001,
    /// HIPAA
    Hipaa,
    /// GDPR
    Gdpr,
    /// PCI DSS
    PciDss,
    /// FedRAMP
    FedRamp,
    /// NIST 800-53
    Nist800_53,
    /// CIS Controls
    CisControls,
    /// CCPA
    Ccpa,
    /// Custom framework
    Custom,
}

impl ComplianceFramework {
    /// Get framework display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Soc2Type2 => "SOC 2 Type II",
            Self::Iso27001 => "ISO/IEC 27001:2022",
            Self::Hipaa => "HIPAA",
            Self::Gdpr => "GDPR",
            Self::PciDss => "PCI DSS v4.0",
            Self::FedRamp => "FedRAMP",
            Self::Nist800_53 => "NIST SP 800-53 Rev 5",
            Self::CisControls => "CIS Controls v8",
            Self::Ccpa => "CCPA",
            Self::Custom => "Custom",
        }
    }

    /// Get framework description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Soc2Type2 => "Service Organization Control 2 - Security, availability, processing integrity, confidentiality, and privacy",
            Self::Iso27001 => "Information Security Management System standard",
            Self::Hipaa => "Health Insurance Portability and Accountability Act",
            Self::Gdpr => "General Data Protection Regulation",
            Self::PciDss => "Payment Card Industry Data Security Standard",
            Self::FedRamp => "Federal Risk and Authorization Management Program",
            Self::Nist800_53 => "Security and Privacy Controls for Information Systems",
            Self::CisControls => "Center for Internet Security Critical Security Controls",
            Self::Ccpa => "California Consumer Privacy Act",
            Self::Custom => "Custom compliance framework",
        }
    }
}

/// Compliance control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceControl {
    /// Control ID
    pub id: String,
    /// Control name
    pub name: String,
    /// Control description
    pub description: String,
    /// Control category
    pub category: String,
    /// Implementation guidance
    pub guidance: String,
    /// Evidence requirements
    pub evidence_requirements: Vec<String>,
    /// Related controls
    pub related_controls: Vec<String>,
    /// Risk level if not implemented
    pub risk_level: RiskLevel,
}

/// Risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Low risk
    Low,
    /// Medium risk
    Medium,
    /// High risk
    High,
    /// Critical risk
    Critical,
}

/// Compliance template for a framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceTemplate {
    /// Framework
    pub framework: ComplianceFramework,
    /// Template version
    pub version: String,
    /// Template description
    pub description: String,
    /// Controls
    pub controls: Vec<ComplianceControl>,
    /// Control categories
    pub categories: Vec<String>,
    /// Mapping to other frameworks
    pub framework_mappings: HashMap<String, Vec<String>>,
}

impl ComplianceTemplate {
    /// Load SOC 2 Type II template
    pub fn soc2_type2() -> Self {
        Self {
            framework: ComplianceFramework::Soc2Type2,
            version: "2.0".to_string(),
            description: "SOC 2 Type II compliance controls".to_string(),
            categories: vec![
                "Security".to_string(),
                "Availability".to_string(),
                "Processing Integrity".to_string(),
                "Confidentiality".to_string(),
                "Privacy".to_string(),
            ],
            controls: vec![
                ComplianceControl {
                    id: "CC6.1".to_string(),
                    name: "Logical and Physical Access".to_string(),
                    description: "The entity implements logical access security software, infrastructure, and architectures over protected information assets".to_string(),
                    category: "Security".to_string(),
                    guidance: "Implement access controls, authentication mechanisms, and authorization policies".to_string(),
                    evidence_requirements: vec![
                        "Access control policies".to_string(),
                        "Authentication system configurations".to_string(),
                        "User access reviews".to_string(),
                        "Access logs and monitoring".to_string(),
                    ],
                    related_controls: vec!["CC6.2".to_string(), "CC6.3".to_string()],
                    risk_level: RiskLevel::High,
                },
                ComplianceControl {
                    id: "CC6.2".to_string(),
                    name: "System Account Management".to_string(),
                    description: "Prior to issuing system credentials and granting system access, the entity registers and authorizes new internal and external users".to_string(),
                    category: "Security".to_string(),
                    guidance: "Establish a formal process for user account provisioning and deprovisioning".to_string(),
                    evidence_requirements: vec![
                        "Account management procedures".to_string(),
                        "Approval workflows".to_string(),
                        "Access request forms".to_string(),
                    ],
                    related_controls: vec!["CC6.1".to_string(), "CC6.3".to_string()],
                    risk_level: RiskLevel::High,
                },
                ComplianceControl {
                    id: "CC6.6".to_string(),
                    name: "Security Incident Management".to_string(),
                    description: "The entity implements incident detection, response, and recovery procedures".to_string(),
                    category: "Security".to_string(),
                    guidance: "Establish an incident response plan with defined roles, procedures, and communication channels".to_string(),
                    evidence_requirements: vec![
                        "Incident response plan".to_string(),
                        "Incident log".to_string(),
                        "Post-incident reports".to_string(),
                    ],
                    related_controls: vec!["CC7.1".to_string(), "CC7.2".to_string()],
                    risk_level: RiskLevel::Critical,
                },
                ComplianceControl {
                    id: "CC7.2".to_string(),
                    name: "System Monitoring".to_string(),
                    description: "The entity monitors system components and the operation of those components".to_string(),
                    category: "Security".to_string(),
                    guidance: "Implement logging, monitoring, and alerting for security-relevant events".to_string(),
                    evidence_requirements: vec![
                        "Logging configuration".to_string(),
                        "Monitoring dashboards".to_string(),
                        "Alert rules".to_string(),
                    ],
                    related_controls: vec!["CC7.1".to_string(), "CC7.4".to_string()],
                    risk_level: RiskLevel::High,
                },
                ComplianceControl {
                    id: "CC8.1".to_string(),
                    name: "Change Management".to_string(),
                    description: "The entity authorizes, designs, develops or acquires, configures, documents, tests, approves, and implements changes".to_string(),
                    category: "Processing Integrity".to_string(),
                    guidance: "Establish a change management process with proper approvals and testing".to_string(),
                    evidence_requirements: vec![
                        "Change management policy".to_string(),
                        "Change request forms".to_string(),
                        "Test results".to_string(),
                        "Deployment records".to_string(),
                    ],
                    related_controls: vec!["CC8.2".to_string()],
                    risk_level: RiskLevel::Medium,
                },
            ],
            framework_mappings: HashMap::new(),
        }
    }

    /// Load HIPAA template
    pub fn hipaa() -> Self {
        Self {
            framework: ComplianceFramework::Hipaa,
            version: "1.0".to_string(),
            description: "HIPAA Security Rule compliance controls".to_string(),
            categories: vec![
                "Administrative Safeguards".to_string(),
                "Physical Safeguards".to_string(),
                "Technical Safeguards".to_string(),
                "Organizational Requirements".to_string(),
            ],
            controls: vec![
                ComplianceControl {
                    id: "164.312(a)(1)".to_string(),
                    name: "Access Control".to_string(),
                    description: "Implement technical policies and procedures for electronic information systems that maintain ePHI".to_string(),
                    category: "Technical Safeguards".to_string(),
                    guidance: "Implement access controls including unique user identification, emergency access procedures, automatic logoff, and encryption".to_string(),
                    evidence_requirements: vec![
                        "Access control policies".to_string(),
                        "User access matrix".to_string(),
                        "Encryption configuration".to_string(),
                    ],
                    related_controls: vec!["164.312(d)".to_string()],
                    risk_level: RiskLevel::Critical,
                },
                ComplianceControl {
                    id: "164.312(b)".to_string(),
                    name: "Audit Controls".to_string(),
                    description: "Implement hardware, software, and/or procedural mechanisms that record and examine activity".to_string(),
                    category: "Technical Safeguards".to_string(),
                    guidance: "Implement audit logging and regular review of audit records".to_string(),
                    evidence_requirements: vec![
                        "Audit log configuration".to_string(),
                        "Log retention policy".to_string(),
                        "Audit review procedures".to_string(),
                    ],
                    related_controls: vec!["164.308(a)(1)(ii)(D)".to_string()],
                    risk_level: RiskLevel::High,
                },
                ComplianceControl {
                    id: "164.312(c)(1)".to_string(),
                    name: "Integrity".to_string(),
                    description: "Implement policies and procedures to protect ePHI from improper alteration or destruction".to_string(),
                    category: "Technical Safeguards".to_string(),
                    guidance: "Implement integrity controls including hashing, digital signatures, and access controls".to_string(),
                    evidence_requirements: vec![
                        "Data integrity policies".to_string(),
                        "Hashing implementations".to_string(),
                        "Backup verification".to_string(),
                    ],
                    related_controls: vec!["164.312(e)(1)".to_string()],
                    risk_level: RiskLevel::High,
                },
                ComplianceControl {
                    id: "164.312(d)".to_string(),
                    name: "Person or Entity Authentication".to_string(),
                    description: "Implement procedures to verify that a person or entity seeking access to ePHI is the one claimed".to_string(),
                    category: "Technical Safeguards".to_string(),
                    guidance: "Implement authentication mechanisms including multi-factor authentication".to_string(),
                    evidence_requirements: vec![
                        "Authentication policies".to_string(),
                        "MFA configuration".to_string(),
                        "Password policies".to_string(),
                    ],
                    related_controls: vec!["164.312(a)(1)".to_string()],
                    risk_level: RiskLevel::Critical,
                },
                ComplianceControl {
                    id: "164.312(e)(1)".to_string(),
                    name: "Transmission Security".to_string(),
                    description: "Implement technical security measures to guard against unauthorized access to ePHI being transmitted".to_string(),
                    category: "Technical Safeguards".to_string(),
                    guidance: "Implement encryption in transit using TLS 1.2 or higher".to_string(),
                    evidence_requirements: vec![
                        "TLS configuration".to_string(),
                        "Certificate management".to_string(),
                        "Network security controls".to_string(),
                    ],
                    related_controls: vec!["164.312(c)(1)".to_string()],
                    risk_level: RiskLevel::Critical,
                },
            ],
            framework_mappings: HashMap::new(),
        }
    }

    /// Load GDPR template
    pub fn gdpr() -> Self {
        Self {
            framework: ComplianceFramework::Gdpr,
            version: "1.0".to_string(),
            description: "GDPR compliance controls".to_string(),
            categories: vec![
                "Data Subject Rights".to_string(),
                "Data Protection".to_string(),
                "Data Processing".to_string(),
                "Cross-Border Transfers".to_string(),
            ],
            controls: vec![
                ComplianceControl {
                    id: "Art.5".to_string(),
                    name: "Principles relating to processing of personal data".to_string(),
                    description: "Personal data shall be processed lawfully, fairly and in a transparent manner".to_string(),
                    category: "Data Processing".to_string(),
                    guidance: "Implement data processing principles including lawfulness, purpose limitation, data minimization, accuracy, storage limitation, and integrity".to_string(),
                    evidence_requirements: vec![
                        "Data processing register".to_string(),
                        "Privacy notices".to_string(),
                        "Retention policies".to_string(),
                    ],
                    related_controls: vec!["Art.6".to_string(), "Art.7".to_string()],
                    risk_level: RiskLevel::High,
                },
                ComplianceControl {
                    id: "Art.32".to_string(),
                    name: "Security of processing".to_string(),
                    description: "Implement appropriate technical and organizational measures to ensure a level of security appropriate to the risk".to_string(),
                    category: "Data Protection".to_string(),
                    guidance: "Implement security measures including encryption, access controls, and incident response".to_string(),
                    evidence_requirements: vec![
                        "Security policies".to_string(),
                        "Encryption standards".to_string(),
                        "Access control implementation".to_string(),
                    ],
                    related_controls: vec!["Art.33".to_string(), "Art.34".to_string()],
                    risk_level: RiskLevel::Critical,
                },
                ComplianceControl {
                    id: "Art.33".to_string(),
                    name: "Notification of a personal data breach to the supervisory authority".to_string(),
                    description: "Notify the supervisory authority within 72 hours of becoming aware of a breach".to_string(),
                    category: "Data Protection".to_string(),
                    guidance: "Implement breach detection, investigation, and notification procedures".to_string(),
                    evidence_requirements: vec![
                        "Breach notification procedures".to_string(),
                        "Breach register".to_string(),
                        "Communication templates".to_string(),
                    ],
                    related_controls: vec!["Art.32".to_string(), "Art.34".to_string()],
                    risk_level: RiskLevel::Critical,
                },
            ],
            framework_mappings: HashMap::new(),
        }
    }

    /// Get control by ID
    pub fn get_control(&self, id: &str) -> Option<&ComplianceControl> {
        self.controls.iter().find(|c| c.id == id)
    }

    /// Get controls by category
    pub fn get_controls_by_category(&self, category: &str) -> Vec<&ComplianceControl> {
        self.controls
            .iter()
            .filter(|c| c.category == category)
            .collect()
    }
}

/// Compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Report ID
    pub id: String,
    /// Framework
    pub framework: ComplianceFramework,
    /// Report date
    pub report_date: chrono::DateTime<chrono::Utc>,
    /// Reporting period
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
    /// Control assessments
    pub assessments: Vec<ControlAssessment>,
    /// Overall compliance score
    pub compliance_score: f32,
    /// Findings
    pub findings: Vec<ComplianceFinding>,
    /// Recommendations
    pub recommendations: Vec<String>,
    /// Attestation
    pub attestation: Option<Attestation>,
}

/// Control assessment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAssessment {
    /// Control ID
    pub control_id: String,
    /// Control name
    pub control_name: String,
    /// Compliance status
    pub status: ComplianceStatus,
    /// Evidence collected
    pub evidence: Vec<Evidence>,
    /// Test results
    pub test_results: Vec<TestResult>,
    /// Gap description (if any)
    pub gap_description: Option<String>,
    /// Remediation plan (if needed)
    pub remediation_plan: Option<String>,
}

/// Compliance status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceStatus {
    /// Fully compliant
    Compliant,
    /// Partially compliant
    PartiallyCompliant,
    /// Non-compliant
    NonCompliant,
    /// Not applicable
    NotApplicable,
    /// Not tested
    NotTested,
}

/// Evidence collected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Evidence ID
    pub id: String,
    /// Evidence type
    pub evidence_type: EvidenceType,
    /// Description
    pub description: String,
    /// File path or URL
    pub location: Option<String>,
    /// Collection date
    pub collected_at: chrono::DateTime<chrono::Utc>,
    /// Collected by
    pub collected_by: String,
}

/// Evidence type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceType {
    /// Policy document
    Policy,
    /// Procedure document
    Procedure,
    /// System configuration
    Configuration,
    /// Log file
    Log,
    /// Screenshot
    Screenshot,
    /// Interview notes
    Interview,
    /// Test output
    TestOutput,
    /// Third-party attestation
    Attestation,
}

/// Test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Test name
    pub test_name: String,
    /// Test description
    pub description: String,
    /// Pass/Fail
    pub passed: bool,
    /// Details
    pub details: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Compliance finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceFinding {
    /// Finding ID
    pub id: String,
    /// Related control
    pub control_id: String,
    /// Finding title
    pub title: String,
    /// Description
    pub description: String,
    /// Severity
    pub severity: RiskLevel,
    /// Remediation deadline
    pub remediation_deadline: Option<chrono::DateTime<chrono::Utc>>,
    /// Remediation status
    pub remediation_status: RemediationStatus,
}

/// Remediation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemediationStatus {
    /// Not started
    NotStarted,
    /// In progress
    InProgress,
    /// Completed
    Completed,
    /// Accepted risk
    AcceptedRisk,
}

/// Attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Attestor name
    pub name: String,
    /// Attestor title
    pub title: String,
    /// Attestation date
    pub date: chrono::DateTime<chrono::Utc>,
    /// Statement
    pub statement: String,
    /// Signature (base64)
    pub signature: Option<String>,
}

/// Compliance manager
pub struct ComplianceManager {
    templates: HashMap<ComplianceFramework, ComplianceTemplate>,
    reports: Vec<ComplianceReport>,
}

impl ComplianceManager {
    /// Create a new compliance manager
    pub fn new() -> Self {
        let mut templates = HashMap::new();
        templates.insert(
            ComplianceFramework::Soc2Type2,
            ComplianceTemplate::soc2_type2(),
        );
        templates.insert(ComplianceFramework::Hipaa, ComplianceTemplate::hipaa());
        templates.insert(ComplianceFramework::Gdpr, ComplianceTemplate::gdpr());

        Self {
            templates,
            reports: Vec::new(),
        }
    }

    /// Get available frameworks
    pub fn available_frameworks(&self) -> Vec<ComplianceFramework> {
        self.templates.keys().copied().collect()
    }

    /// Get a template
    pub fn get_template(&self, framework: ComplianceFramework) -> Option<&ComplianceTemplate> {
        self.templates.get(&framework)
    }

    /// Generate a compliance report
    pub fn generate_report(
        &mut self,
        framework: ComplianceFramework,
        period_start: chrono::DateTime<chrono::Utc>,
        period_end: chrono::DateTime<chrono::Utc>,
    ) -> Result<ComplianceReport> {
        let template = self
            .templates
            .get(&framework)
            .ok_or_else(|| anyhow::anyhow!("Framework template not found"))?;

        let assessments: Vec<ControlAssessment> = template
            .controls
            .iter()
            .map(|control| ControlAssessment {
                control_id: control.id.clone(),
                control_name: control.name.clone(),
                status: ComplianceStatus::NotTested,
                evidence: Vec::new(),
                test_results: Vec::new(),
                gap_description: None,
                remediation_plan: None,
            })
            .collect();

        let compliant_count = assessments
            .iter()
            .filter(|a| a.status == ComplianceStatus::Compliant)
            .count();
        let total_applicable = assessments
            .iter()
            .filter(|a| a.status != ComplianceStatus::NotApplicable)
            .count();
        let compliance_score = if total_applicable > 0 {
            (compliant_count as f32 / total_applicable as f32) * 100.0
        } else {
            0.0
        };

        let report = ComplianceReport {
            id: uuid::Uuid::new_v4().to_string(),
            framework,
            report_date: chrono::Utc::now(),
            period_start,
            period_end,
            assessments,
            compliance_score,
            findings: Vec::new(),
            recommendations: Vec::new(),
            attestation: None,
        };

        self.reports.push(report.clone());
        Ok(report)
    }

    /// Get reports
    pub fn get_reports(&self) -> &[ComplianceReport] {
        &self.reports
    }

    /// Update control assessment
    pub fn update_assessment(
        &mut self,
        report_id: &str,
        control_id: &str,
        status: ComplianceStatus,
        evidence: Vec<Evidence>,
    ) -> Result<()> {
        let report = self
            .reports
            .iter_mut()
            .find(|r| r.id == report_id)
            .ok_or_else(|| anyhow::anyhow!("Report not found"))?;

        if let Some(assessment) = report
            .assessments
            .iter_mut()
            .find(|a| a.control_id == control_id)
        {
            assessment.status = status;
            assessment.evidence = evidence;
        }

        // Recalculate score
        let compliant_count = report
            .assessments
            .iter()
            .filter(|a| a.status == ComplianceStatus::Compliant)
            .count();
        let total_applicable = report
            .assessments
            .iter()
            .filter(|a| a.status != ComplianceStatus::NotApplicable)
            .count();
        report.compliance_score = if total_applicable > 0 {
            (compliant_count as f32 / total_applicable as f32) * 100.0
        } else {
            0.0
        };

        Ok(())
    }
}

impl Default for ComplianceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soc2_template() {
        let template = ComplianceTemplate::soc2_type2();
        assert_eq!(template.framework, ComplianceFramework::Soc2Type2);
        assert!(!template.controls.is_empty());
    }

    #[test]
    fn test_get_control_by_id() {
        let template = ComplianceTemplate::soc2_type2();
        let control = template.get_control("CC6.1");
        assert!(control.is_some());
    }

    #[test]
    fn test_compliance_manager() {
        let manager = ComplianceManager::new();
        assert!(!manager.available_frameworks().is_empty());
    }

    #[test]
    fn test_generate_report() {
        let mut manager = ComplianceManager::new();
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::days(365);

        let report = manager.generate_report(ComplianceFramework::Soc2Type2, start, now);

        assert!(report.is_ok());
    }
}
