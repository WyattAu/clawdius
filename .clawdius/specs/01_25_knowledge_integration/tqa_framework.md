# Translation Quality Assurance (TQA) Framework

## Overview

The TQA Framework provides a systematic approach to evaluating and certifying multi-lingual documentation quality across all Clawdius specifications.

## TQA Level Definitions

| Level | Method | Confidence | Use Case | Review Requirements |
|-------|--------|------------|----------|---------------------|
| 1 | Machine Translation (MT) | 0.0-0.4 | Initial screening | None - automated only |
| 2 | Back-Translation (BT) | 0.4-0.6 | Preliminary understanding | 1 automated validation |
| 3 | Technical Review (TR) | 0.6-0.8 | Technical analysis | 1 domain expert |
| 4 | Peer Validation (PV) | 0.8-0.9 | Critical decisions | 2 domain experts |
| 5 | Expert Consensus (EC) | 0.9-1.0 | Safety-critical | 3+ experts + native speaker |

## Quality Scoring Methodology

### Dimension Scores (0-10 scale)

1. **Semantic Accuracy (SA)**: Meaning preservation from source
2. **Technical Terminology (TT)**: Correct use of domain-specific terms
3. **Grammatical Correctness (GC)**: Grammar and syntax accuracy
4. **Cultural Appropriateness (CA)**: Cultural localization quality
5. **Readability (R)**: Natural flow and comprehensibility

### Composite Score Calculation

```
TQA_Score = (SA × 0.30) + (TT × 0.25) + (GC × 0.20) + (CA × 0.15) + (R × 0.10)
```

### Confidence Interval

```
Confidence = Base_Confidence + (TQA_Score × 0.06)
```

## Level Requirements by Document Type

| Document Type | Minimum Level | Minimum Score |
|---------------|---------------|---------------|
| API Reference | Level 3 | 7.0 |
| Architecture Spec | Level 4 | 8.0 |
| Security Protocol | Level 5 | 9.0 |
| User Guide | Level 2 | 5.0 |
| Code Comments | Level 2 | 5.0 |
| Error Messages | Level 3 | 7.0 |

## Language Support Matrix

| Code | Language | Native Script | L1 | L2 | L3 | L4 | L5 | Primary Reviewer |
|------|----------|---------------|----|----|----|----|----|------------------|
| EN | English | English | ✓ | ✓ | ✓ | ✓ | ✓ | Default |
| ZH | Chinese | 中文 | ✓ | ✓ | ✓ | ✓ | ✓ | zh_reviewer |
| RU | Russian | Русский | ✓ | ✓ | ✓ | ✓ | ✓ | ru_reviewer |
| DE | German | Deutsch | ✓ | ✓ | ✓ | ✓ | ○ | de_reviewer |
| FR | French | Français | ✓ | ✓ | ✓ | ✓ | ○ | fr_reviewer |
| JP | Japanese | 日本語 | ✓ | ✓ | ✓ | ✓ | ✓ | jp_reviewer |
| KO | Korean | 한국어 | ✓ | ✓ | ✓ | ✓ | ✓ | ko_reviewer |
| ES | Spanish | Español | ✓ | ✓ | ✓ | ✓ | ○ | es_reviewer |
| IT | Italian | Italiano | ✓ | ✓ | ✓ | ○ | ○ | it_reviewer |
| PT | Portuguese | Português | ✓ | ✓ | ✓ | ○ | ○ | pt_reviewer |
| NL | Dutch | Nederlands | ✓ | ✓ | ○ | ○ | ○ | nl_reviewer |
| PL | Polish | Polski | ✓ | ✓ | ○ | ○ | ○ | pl_reviewer |
| CS | Czech | Čeština | ✓ | ✓ | ○ | ○ | ○ | cs_reviewer |
| AR | Arabic | العربية | ✓ | ✓ | ✓ | ○ | ○ | ar_reviewer |
| FA | Persian | فارسی | ✓ | ✓ | ○ | ○ | ○ | fa_reviewer |
| TR | Turkish | Türkçe | ✓ | ✓ | ○ | ○ | ○ | tr_reviewer |

Legend: ✓ = Fully supported, ○ = Limited support

## TQA Process Workflow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Source    │────▶│  L1: MT     │────▶│  L2: BT     │
│  Document   │     │  Screening  │     │ Validation  │
└─────────────┘     └─────────────┘     └─────────────┘
                                               │
                    ┌─────────────┐             │
                    │  L5: Expert │◀────────────┤
                    │  Consensus  │             │
                    └─────────────┘             │
                          ▲                     │
                          │              ┌──────┴──────┐
                    ┌─────┴─────┐        │             │
                    │  L4: Peer │◀───────┤             │
                    │ Validation│        │             │
                    └───────────┘        ▼             │
                                         │             │
                    ┌─────────────┐      │             │
                    │  L3: Tech   │◀─────┘             │
                    │   Review    │                    │
                    └─────────────┘                    │
                          │                           │
                          ▼                           │
                    ┌─────────────┐                   │
                    │  Certified  │◀──────────────────┘
                    │ Translation │
                    └─────────────┘
```

## Terminology Alignment

All translations must align with the canonical terminology defined in:
- `.clawdius/knowledge_graph/terminology.json`

### Terminology Validation Rules

1. **Mandatory**: Technical terms MUST use canonical translations
2. **Preferred**: Domain-specific terms SHOULD use terminology.json entries
3. **Contextual**: Ambiguous terms require context-specific resolution
4. **New Terms**: Must be submitted for terminology review before translation

## Reviewer Certification Requirements

| Level | Requirements |
|-------|--------------|
| L3 Reviewer | Domain expertise + B2+ target language |
| L4 Reviewer | Domain expertise + C1+ target language + 2yr experience |
| L5 Reviewer | Domain expertise + Native/C2 + 5yr experience + certification |

## Quality Metrics Dashboard

### Key Performance Indicators

- **Translation Coverage**: % of documents with TQA certification
- **Average Score**: Mean TQA_Score across all translations
- **Terminology Compliance**: % of terms using canonical translations
- **Time to Certification**: Average days from submission to certification
- **Revision Rate**: % of translations requiring post-certification updates

### Reporting Frequency

- Weekly: Coverage and average score
- Monthly: Compliance and revision metrics
- Quarterly: Full TQA audit report

## Integration with Knowledge Graph

```json
{
  "tqa_metadata": {
    "document_id": "SPEC-XXX",
    "source_language": "EN",
    "target_language": "ZH",
    "tqa_level": 4,
    "tqa_score": 8.5,
    "certified_date": "2025-01-25",
    "reviewers": ["zh_reviewer_1", "zh_reviewer_2"],
    "terminology_compliance": 0.98
  }
}
```

## Appendix A: Scoring Rubric

### Semantic Accuracy (SA)
| Score | Criteria |
|-------|----------|
| 9-10 | Perfect meaning preservation, no ambiguity |
| 7-8 | Minor nuances may differ, core meaning intact |
| 5-6 | Some meaning loss, but generally accurate |
| 3-4 | Significant meaning drift or omissions |
| 0-2 | Major errors, meaning substantially changed |

### Technical Terminology (TT)
| Score | Criteria |
|-------|----------|
| 9-10 | 100% terminology.json compliance |
| 7-8 | 95%+ compliance, deviations documented |
| 5-6 | 85%+ compliance, some inconsistencies |
| 3-4 | 70%+ compliance, frequent deviations |
| 0-2 | <70% compliance, terminology errors |

### Grammatical Correctness (GC)
| Score | Criteria |
|-------|----------|
| 9-10 | Native-level grammar, no errors |
| 7-8 | Minor errors that don't affect comprehension |
| 5-6 | Noticeable errors, still readable |
| 3-4 | Frequent errors, challenging to read |
| 0-2 | Severe grammatical issues |

### Cultural Appropriateness (CA)
| Score | Criteria |
|-------|----------|
| 9-10 | Fully localized, culturally natural |
| 7-8 | Well-localized with minor adjustments needed |
| 5-6 | Adequate localization, some cultural gaps |
| 3-4 | Limited localization, noticeable foreign feel |
| 0-2 | Poor localization, potentially confusing |

### Readability (R)
| Score | Criteria |
|-------|----------|
| 9-10 | Flows naturally, pleasant to read |
| 7-8 | Generally smooth, minor awkwardness |
| 5-6 | Readable but could be smoother |
| 3-4 | Choppy, requires effort to follow |
| 0-2 | Very difficult to read |

---

*Framework Version: 1.0*
*Last Updated: 2025-01-25*
*Maintained by: DeepThought Knowledge Integration System*
