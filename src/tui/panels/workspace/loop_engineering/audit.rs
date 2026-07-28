fn exists_nonempty(path: &Path) -> bool {
    path.is_file()
        && std::fs::metadata(path)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
}

pub(crate) fn audit_loop(spec: &LoopSpec) -> LoopAudit {
    let mut score = 0u8;
    let mut passed = Vec::new();
    let mut missing = Vec::new();
    let mut warnings = Vec::new();
    let mut add = |points: u8, ok: bool, pass: &str, miss: &str| {
        if ok {
            score = score.saturating_add(points);
            passed.push(pass.to_string());
        } else {
            missing.push(miss.to_string());
        }
    };
    add(
        10,
        !spec.goal.trim().is_empty(),
        "single clear goal",
        "add a single-sentence goal",
    );
    add(
        10,
        exists_nonempty(&spec.dir.join(STATE_FILE)),
        "durable STATE.md",
        "create STATE.md",
    );
    add(
        10,
        exists_nonempty(&spec.dir.join(RUN_LOG_FILE)),
        "append-only RUN_LOG.md",
        "create RUN_LOG.md",
    );
    add(
        10,
        exists_nonempty(&spec.dir.join(BUDGET_FILE)) && spec.budget_tokens_per_day > 0,
        "budget and kill-switch file",
        "create budget.toml with daily caps",
    );
    add(
        10,
        !spec.denylist.is_empty(),
        "denylist paths configured",
        "add denylist paths for secrets/infra",
    );
    add(
        15,
        !spec.maker_agent.is_empty()
            && !spec.checker_agent.is_empty()
            && spec.maker_agent != spec.checker_agent,
        "maker/checker split",
        "configure separate maker_agent and checker_agent",
    );
    let agent_loop = spec.level == "A2";
    let goal_loop = spec.level == "G1";
    add(
        10,
        spec.worktree || agent_loop || goal_loop,
        if agent_loop {
            "agent asset scope requested"
        } else if goal_loop {
            "goal scope is guarded by the active session"
        } else {
            "worktree isolation requested"
        },
        if agent_loop {
            "scope the loop to one agent definition"
        } else {
            "enable worktree isolation before L2"
        },
    );
    add(
        10,
        if agent_loop || goal_loop {
            !spec.os_runtime && !spec.connectors.iter().any(|c| c == "os-runtime")
        } else {
            spec.os_runtime && spec.connectors.iter().any(|c| c == "os-runtime")
        },
        if agent_loop {
            "local agent loop runtime"
        } else if goal_loop {
            "local goal loop runtime"
        } else {
            "OS Runtime connector enabled"
        },
        if agent_loop {
            "disable OS Runtime for local /agent loops"
        } else {
            "enable os_runtime/connectors=[\"os-runtime\"]"
        },
    );
    let skill_pair = if goal_loop {
        exists_nonempty(&spec.dir.join("skills").join("maker.md"))
            && exists_nonempty(&spec.dir.join("skills").join("verifier.md"))
    } else {
        exists_nonempty(&spec.dir.join("skills").join("triage.md"))
            && exists_nonempty(&spec.dir.join("skills").join("verifier.md"))
    };
    add(
        15,
        skill_pair,
        if goal_loop {
            "maker and verifier skills"
        } else {
            "triage and verifier skills"
        },
        if goal_loop {
            "add skills/maker.md and skills/verifier.md"
        } else {
            "add skills/triage.md and skills/verifier.md"
        },
    );
    if spec.level == "L3" && score < 90 {
        warnings.push("L3 requested but readiness is below unattended threshold".to_string());
    }
    if spec.level != "L1" && spec.level != "A2" && spec.level != "G1" && !spec.worktree {
        warnings.push("acting loops should use worktree isolation".to_string());
    }
    let level = if score >= 90 {
        "L3-ready"
    } else if score >= 75 {
        "L2-ready"
    } else if score >= 50 {
        "L1-ready"
    } else {
        "L0-draft"
    }
    .to_string();
    LoopAudit {
        score,
        level,
        passed,
        missing,
        warnings,
    }
}
