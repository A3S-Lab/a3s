//! Built-in, verified OKF starter catalog.
//!
//! The catalog is compiled into A3S Code so the knowledge marketplace has a
//! deterministic offline lifecycle. Remote OS catalogs can be layered on top
//! later without changing the personal knowledge-base storage contract.

#[derive(Clone, Copy, Debug)]
pub(super) struct MarketFile {
    pub(super) path: &'static str,
    pub(super) content: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MarketPackage {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) publisher: &'static str,
    pub(super) version: &'static str,
    pub(super) category: &'static str,
    pub(super) tags: &'static [&'static str],
    pub(super) featured: bool,
    pub(super) updated_at: &'static str,
    pub(super) files: &'static [MarketFile],
}

impl MarketPackage {
    pub(super) fn source_count(self) -> usize {
        self.files
            .iter()
            .filter(|file| file.path.starts_with("sources/"))
            .count()
    }

    pub(super) fn concept_count(self) -> usize {
        self.files
            .iter()
            .filter(|file| file.path.starts_with("wiki/") && file.path.ends_with(".md"))
            .count()
    }
}

const PRODUCT_GUIDE_FILES: &[MarketFile] = &[
    MarketFile {
        path: "README.md",
        content: "# A3S Product Guide\n\nA concise guide to A3S product concepts, workflows, and local-first operating principles.\n",
    },
    MarketFile {
        path: "sources/product-overview.md",
        content: "# Product Overview\n\nA3S combines coding, office work, memory, verified plugins, and portable knowledge packages in one local-first workspace.\n",
    },
    MarketFile {
        path: "sources/marketplace-safety.md",
        content: "# Marketplace Safety\n\nExecutable plugins and data-only knowledge packages use separate installation paths. Package identity and provenance remain visible after installation.\n",
    },
    MarketFile {
        path: "wiki/index.md",
        content: "# A3S Product Guide\n\n- [Local-first workspace](concepts/local-first.md)\n- [Capability boundaries](concepts/capability-boundaries.md)\n",
    },
    MarketFile {
        path: "wiki/concepts/local-first.md",
        content: "# Local-first workspace\n\nUser-created work and personal knowledge remain inspectable on the local filesystem.\n",
    },
    MarketFile {
        path: "wiki/concepts/capability-boundaries.md",
        content: "# Capability boundaries\n\nBuilt-in products, executable plugins, and knowledge packages have explicit and different lifecycle boundaries.\n",
    },
    MarketFile {
        path: "eval/smoke.md",
        content: "# Smoke Evaluation\n\n1. Confirm all links in `wiki/index.md` resolve.\n2. Confirm product and security claims have a source page.\n",
    },
];

const RESEARCH_METHODS_FILES: &[MarketFile] = &[
    MarketFile {
        path: "README.md",
        content: "# Research Methods Starter\n\nA discipline-neutral starter for framing questions, recording evidence, and reviewing conclusions.\n",
    },
    MarketFile {
        path: "sources/research-cycle.md",
        content: "# Research Cycle\n\nFrame a bounded question, define evidence criteria, collect traceable sources, synthesize findings, and record limitations.\n",
    },
    MarketFile {
        path: "sources/evidence-ledger.md",
        content: "# Evidence Ledger\n\nRecord source identity, retrieval date, claim support, scope, and unresolved conflicts for every material finding.\n",
    },
    MarketFile {
        path: "wiki/index.md",
        content: "# Research Methods\n\n- [Question framing](concepts/question-framing.md)\n- [Evidence review](concepts/evidence-review.md)\n",
    },
    MarketFile {
        path: "wiki/concepts/question-framing.md",
        content: "# Question framing\n\nA useful research question declares its target, scope, time boundary, and completion criteria.\n",
    },
    MarketFile {
        path: "wiki/concepts/evidence-review.md",
        content: "# Evidence review\n\nReview provenance, relevance, recency, and contradiction before accepting a source-backed claim.\n",
    },
    MarketFile {
        path: "eval/smoke.md",
        content: "# Smoke Evaluation\n\n1. Trace each conclusion to evidence.\n2. Preserve conflicting evidence and explicit limitations.\n",
    },
];

const ENGINEERING_PLAYBOOK_FILES: &[MarketFile] = &[
    MarketFile {
        path: "README.md",
        content: "# Software Engineering Playbook\n\nPractical guidance for scoped changes, reviewable code, and evidence-backed delivery.\n",
    },
    MarketFile {
        path: "sources/change-lifecycle.md",
        content: "# Change Lifecycle\n\nUnderstand the owning layer, write a focused regression test, implement the smallest coherent change, and verify the affected workflow.\n",
    },
    MarketFile {
        path: "sources/review-checklist.md",
        content: "# Review Checklist\n\nCheck correctness, failure behavior, ownership, compatibility, security boundaries, tests, and documentation.\n",
    },
    MarketFile {
        path: "wiki/index.md",
        content: "# Engineering Playbook\n\n- [Scoped change](concepts/scoped-change.md)\n- [Verification evidence](concepts/verification-evidence.md)\n",
    },
    MarketFile {
        path: "wiki/concepts/scoped-change.md",
        content: "# Scoped change\n\nA scoped change modifies one product contract and avoids unrelated cleanup or speculative abstractions.\n",
    },
    MarketFile {
        path: "wiki/concepts/verification-evidence.md",
        content: "# Verification evidence\n\nVerification records the exact checks that exercised the changed behavior and their observable outcomes.\n",
    },
    MarketFile {
        path: "eval/smoke.md",
        content: "# Smoke Evaluation\n\n1. Confirm examples remain executable.\n2. Confirm every recommended check names an observable result.\n",
    },
];

const TEAM_OPERATIONS_FILES: &[MarketFile] = &[
    MarketFile {
        path: "README.md",
        content: "# Team Operations Handbook\n\nA lightweight knowledge structure for decisions, runbooks, incidents, and ownership.\n",
    },
    MarketFile {
        path: "sources/decision-records.md",
        content: "# Decision Records\n\nRecord context, decision, alternatives, consequences, owner, and review date for material team decisions.\n",
    },
    MarketFile {
        path: "sources/runbook-template.md",
        content: "# Runbook Template\n\nState the trigger, prerequisites, safe steps, verification, rollback, escalation, and owner.\n",
    },
    MarketFile {
        path: "wiki/index.md",
        content: "# Team Operations\n\n- [Operational ownership](concepts/operational-ownership.md)\n- [Recoverable procedure](concepts/recoverable-procedure.md)\n",
    },
    MarketFile {
        path: "wiki/concepts/operational-ownership.md",
        content: "# Operational ownership\n\nEvery durable process identifies an accountable owner and an escalation path.\n",
    },
    MarketFile {
        path: "wiki/concepts/recoverable-procedure.md",
        content: "# Recoverable procedure\n\nA recoverable procedure includes a verification point and a bounded rollback path.\n",
    },
    MarketFile {
        path: "eval/smoke.md",
        content: "# Smoke Evaluation\n\n1. Confirm every runbook has an owner.\n2. Confirm mutating steps include verification and rollback.\n",
    },
];

const PACKAGES: &[MarketPackage] = &[
    MarketPackage {
        id: "a3s-product-guide",
        name: "A3S Product Guide",
        description: "Product concepts, local-first workflows, and capability boundaries.",
        publisher: "A3S Lab",
        version: "1.0.0",
        category: "Productivity",
        tags: &["A3S", "Guide"],
        featured: true,
        updated_at: "2026-07-22T00:00:00Z",
        files: PRODUCT_GUIDE_FILES,
    },
    MarketPackage {
        id: "research-methods-starter",
        name: "Research Methods Starter",
        description: "Discipline-neutral question framing, evidence review, and limitations.",
        publisher: "A3S Lab",
        version: "1.0.0",
        category: "Research",
        tags: &["Research", "Evidence"],
        featured: true,
        updated_at: "2026-07-22T00:00:00Z",
        files: RESEARCH_METHODS_FILES,
    },
    MarketPackage {
        id: "software-engineering-playbook",
        name: "Software Engineering Playbook",
        description: "Scoped delivery, review practices, and verification evidence.",
        publisher: "A3S Lab",
        version: "1.0.0",
        category: "Engineering",
        tags: &["Engineering", "Quality"],
        featured: false,
        updated_at: "2026-07-22T00:00:00Z",
        files: ENGINEERING_PLAYBOOK_FILES,
    },
    MarketPackage {
        id: "team-operations-handbook",
        name: "Team Operations Handbook",
        description: "Decision records, runbooks, recoverability, and ownership.",
        publisher: "A3S Lab",
        version: "1.0.0",
        category: "Operations",
        tags: &["Operations", "Runbooks"],
        featured: false,
        updated_at: "2026-07-22T00:00:00Z",
        files: TEAM_OPERATIONS_FILES,
    },
];

pub(super) fn packages() -> &'static [MarketPackage] {
    PACKAGES
}

pub(super) fn package(id: &str) -> Option<MarketPackage> {
    PACKAGES.iter().copied().find(|package| package.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Component, Path};

    use super::*;

    #[test]
    fn catalog_has_unique_safe_ids_and_okf_layouts() {
        let mut ids = HashSet::new();
        for package in packages() {
            assert!(ids.insert(package.id));
            assert!(package
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'));
            assert!(package.files.iter().any(|file| file.path == "README.md"));
            assert!(package
                .files
                .iter()
                .any(|file| file.path.starts_with("sources/")));
            assert!(package
                .files
                .iter()
                .any(|file| file.path.starts_with("wiki/")));
            assert!(package.files.iter().all(|file| {
                Path::new(file.path)
                    .components()
                    .all(|component| matches!(component, Component::Normal(_)))
            }));
        }
    }
}
