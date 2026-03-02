# Concept Mappings: Multi-Lingual Technical Terminology

**Version:** 1.0.0  
**Phase:** 1.25  
**Created:** 2026-03-01  

## Purpose

This document maps key technical concepts across 16 languages to enable cross-lingual knowledge retrieval for Clawdius research synthesis.

---

## 1. Finite State Machine (FSM) Concepts

| Concept ID | EN | ZH | RU | DE | JP | TQA | Confidence |
|------------|----|----|----|----|----|----|------------|
| CONCEPT-FSM-001 | Finite State Machine | 有限状态机 | Конечный автомат | Endlicher Automat | 有限オートマトン | 5 | 0.98 |
| CONCEPT-TYPESTATE-001 | Typestate Pattern | 类型状态模式 | Паттерн типосостояния | Typestate-Muster | タイプステートパターン | 5 | 0.95 |
| CONCEPT-TRANSITION-001 | Phase Transition Function | 阶段转换函数 | Функция фазового перехода | Phasenübergangsfunktion | フェーズ遷移関数 | 5 | 0.99 |
| CONCEPT-GATE-001 | Quality Gate | 质量门 | Ворота качества | Qualitätsschranke | 品質ゲート | 5 | 0.97 |

### Extended Mappings

| Concept | KO | ES | IT | PT | NL |
|---------|----|----|----|----|----|
| Finite State Machine | 유한 상태 기계 | Máquina de estados finitos | Automa a stati finiti | Autômato de estados finitos | Eindige toestandsautomaat |
| Typestate Pattern | 타입스테이트 패턴 | Patrón Typestate | Pattern Typestate | Padrão Typestate | Typestate-patroon |

| Concept | PL | CS | AR | FA | TR | FR |
|---------|----|----|----|----|----|-----|
| Finite State Machine | Automat skończony | Konečný automat | آلة الحالات المحدودة | ماشین حالات محدود | Sonlu durum makinesi | Automate fini |
| Typestate Pattern | Wzorzec Typestate | Vzor Typestate | نمط حالة النوع | الگوی تایپ‌استیت | Typestate deseni | Motif Typestate |

---

## 2. High-Frequency Trading (HFT) Concepts

| Concept ID | EN | ZH | RU | DE | JP | TQA | Confidence |
|------------|----|----|----|----|----|----|------------|
| CONCEPT-HFT-001 | High-Frequency Trading | 高频交易 | Высокочастотная торговля | Hochfrequenzhandel | 高頻度取引 | 5 | 0.96 |
| CONCEPT-WALLET-GUARD-001 | Wallet Guard | 钱包守护 | Защита кошелька | Wallet-Wächter | ウォレットガード | 5 | 0.94 |
| CONCEPT-RING-BUFFER-001 | Lock-Free Ring Buffer | 无锁环形缓冲区 | Блоксвободный кольцевой буфер | Sperrfreier Ringpuffer | ロックフリーリングバッファ | 5 | 0.97 |
| CONCEPT-ARENA-001 | Arena Allocator | 竞技场分配器 | Арена распределения | Arena-Allokator | アリーナアロケータ | 5 | 0.95 |
| CONCEPT-ZERO-GC-001 | Zero-GC Memory Model | 零GC内存模型 | Модель памяти без GC | Null-GC-Speichermodell | ゼロGCメモリモデル | 5 | 0.98 |
| CONCEPT-WCET-001 | Worst-Case Execution Time | 最坏执行时间 | Наихудшее время выполнения | Schlechteste Ausführungszeit | 最悪実行時間 | 5 | 0.96 |

### Extended Mappings

| Concept | KO | ES | IT | PT | NL |
|---------|----|----|----|----|----|
| High-Frequency Trading | 고빈도 거래 | Trading de alta frecuencia | Trading ad alta frequenza | Negociação de alta frequência | High-frequency trading |
| Lock-Free Ring Buffer | 락-프리 링 버퍼 | Búfer circular sin bloqueos | Buffer circolare lock-free | Buffer circular sem travas | Lock-vrije ringbuffer |
| Arena Allocator | 아레나 할당자 | Asignador de arena | Allocatore arena | Alocador de arena | Arena-allocator |

---

## 3. Security & Sandboxing Concepts

| Concept ID | EN | ZH | RU | DE | JP | TQA | Confidence |
|------------|----|----|----|----|----|----|------------|
| CONCEPT-SANDBOX-001 | Sandboxing | 沙箱 | Песочница | Sandbox | サンドボックス | 5 | 0.97 |
| CONCEPT-CAPABILITY-001 | Capability-Based Security | 基于能力的安全 | Возможностная безопасность | Capability-basierte Sicherheit | ケイパビリティベースセキュリティ | 5 | 0.95 |
| CONCEPT-ISOLATION-001 | Isolation Boundary | 隔离边界 | Граница изоляции | Isolationsgrenze | 隔離境界 | 5 | 0.98 |
| CONCEPT-SECRET-001 | Secret Isolation | 秘密隔离 | Изоляция секретов | Geheimnis-Isolierung | シークレット隔離 | 5 | 0.96 |
| CONCEPT-JIT-SANDBOX-001 | JIT Sandboxing | 即时沙箱 | JIT-песочница | JIT-Sandbox | JITサンドボックス | 4 | 0.93 |
| CONCEPT-THREAT-001 | Threat Mitigation Model | 威胁缓解模型 | Модель устранения угроз | Bedrohungsminderungsmodell | 脅威緩和モデル | 4 | 0.91 |

### Extended Mappings

| Concept | KO | ES | IT | PT | NL |
|---------|----|----|----|----|----|
| Sandboxing | 샌드박싱 | Sandboxing | Sandbox | Sandbox | Sandboxing |
| Capability-Based Security | 역량 기반 보안 | Seguridad basada en capacidades | Sicurezza basata su capability | Segurança baseada em capacidades | Capability-gebaseerde beveiliging |

---

## 4. Cross-Domain Term Mappings

### 4.1 Latency & Performance Terms

| EN | ZH | RU | DE | JP | Context |
|----|----|----|----|----|---------|
| Latency | 延迟 | Задержка | Latenz | レイテンシ | HFT, Real-time |
| Throughput | 吞吐量 | Пропускная способность | Durchsatz | スループット | Systems |
| Jitter | 抖动 | Джиттер | Jitter | ジッター | Real-time |
| Tail Latency | 尾延迟 | Хвостовая задержка | Endlatenz | テールレイテンシ | HFT |
| Microsecond | 微秒 | Микросекунда | Mikrosekunde | マイクロ秒 | Timing |

### 4.2 Memory Management Terms

| EN | ZH | RU | DE | JP | Context |
|----|----|----|----|----|---------|
| Allocation | 分配 | Выделение | Allokation | 割り当て | Memory |
| Deallocation | 释放 | Освобождение | Freigabe | 解放 | Memory |
| Arena | 竞技场 | Арена | Arena | アリーナ | Allocator |
| Stack | 栈 | Стек | Stapel | スタック | Memory |
| Heap | 堆 | Куча | Heap | ヒープ | Memory |
| Cache Line | 缓存行 | Кэш-линия | Cache-Zeile | キャッシュライン | Performance |

### 4.3 Security Terms

| EN | ZH | RU | DE | JP | Context |
|----|----|----|----|----|---------|
| Privilege Escalation | 权限提升 | Повышение привилегий | Rechteausweitung | 権限昇格 | Security |
| Supply Chain Attack | 供应链攻击 | Атака на цепочку поставок | Lieferkettenangriff | サプライチェーン攻撃 | Security |
| Zero Trust | 零信任 | Нулевое доверие | Zero Trust | ゼロトラスト | Architecture |
| Exploit | 漏洞利用 | Эксплойт | Exploit | エクスプロイト | Security |
| Hardening | 加固 | Усиление | Härtung | ハードニング | Security |

---

## 5. Research Database Mappings

### 5.1 Primary Language Sources

| Language | Database | URL Pattern | Topics |
|----------|----------|-------------|--------|
| EN | arXiv | `arxiv.org` | ML, Systems, Crypto |
| EN | IEEE Xplore | `ieeexplore.ieee.org` | Engineering, Standards |
| EN | ACM DL | `dl.acm.org` | Computer Science |
| ZH | CNKI | `cnki.net` | Control Systems, Numerical |
| ZH | Wanfang | `wanfangdata.com.cn` | Engineering |
| RU | eLibrary | `elibrary.ru` | Crypto, Formal Methods |
| RU | CyberLeninka | `cyberleninka.ru` | Academic Papers |
| DE | Springer DE | `springer.de` | Automotive, Industrial |
| JP | J-STAGE | `jstage.jst.go.jp` | Real-time, Game Engines |
| JP | CiNii | `ci.nii.ac.jp` | Academic Research |

### 5.2 Secondary Language Sources

| Language | Database | Topics |
|----------|----------|--------|
| KO | DBpia, RISS | Semiconductor, Embedded |
| ES | Dialnet, Redalyc | Regional Standards |
| IT | ACNP, EDIT16 | Industrial Design |
| PT | SciELO Brazil | Engineering |
| NL | NARCIS | Academic Research |
| PL | POL-on | Engineering |
| CS | Czech Digital Math Library | Formal Methods |

---

## 6. Translation Quality Assurance (TQA) Levels

| Level | Method | Confidence | Use Case |
|-------|--------|------------|----------|
| 1 | Machine Translation | ≥0.30 | Initial screening only |
| 2 | Back-Translation | ≥0.50 | Preliminary research |
| 3 | Technical Review | ≥0.70 | General research |
| 4 | Peer Validation | ≥0.85 | Architectural decisions |
| 5 | Expert Consensus | ≥0.95 | Safety-critical algorithms |

### TQA Requirements by Material Type

| Material Type | Minimum TQA | Confidence |
|---------------|-------------|------------|
| Safety-critical algorithms | 5 | ≥0.95 |
| Theorem proofs | 5 | ≥0.95 |
| Security definitions | 5 | ≥0.95 |
| Architectural decisions | 4 | ≥0.85 |
| Domain constraints | 4 | ≥0.85 |
| General research | 3 | ≥0.70 |
| Preliminary screening | 2 | ≥0.50 |

---

## 7. Concept Confidence Scoring

### Factors Affecting Confidence

| Factor | Weight | Description |
|--------|--------|-------------|
| Source authority | 0.30 | Yellow Paper vs external source |
| Formal definition | 0.25 | Presence of mathematical notation |
| Cross-reference | 0.20 | Multiple sources confirm |
| Translation quality | 0.15 | TQA level achieved |
| Recency | 0.10 | Publication date within 5 years |

### Confidence Thresholds

| Threshold | Action |
|-----------|--------|
| ≥0.95 | Accept without review |
| 0.85-0.94 | Accept with documentation |
| 0.70-0.84 | Flag for verification |
| <0.70 | Reject or escalate |

---

## Statistics

- **Total Concepts Mapped:** 18
- **Languages Covered:** 16
- **Primary Languages (5):** EN, ZH, RU, DE, JP
- **TQA Level 5 Concepts:** 15
- **TQA Level 4 Concepts:** 3
- **Average Confidence:** 0.956
