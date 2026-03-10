//! SafeClaw built-in skills.
//!
//! All skills bundled with SafeClaw are defined here. They are embedded at
//! compile time and registered into the [`SkillRegistry`] during bootstrap.
//!
//! Runtime-conditional skills (e.g., `a3s-box`) are only registered when the
//! corresponding binary is detected at startup or after installation.
//!
//! [`SkillRegistry`]: a3s_code::skills::SkillRegistry

use a3s_code::skills::Skill;
use std::sync::Arc;

/// Embedded skill: A3S Box MicroVM management.
const A3S_BOX_SKILL_MD: &str = include_str!("../skills/a3s-box.md");

/// Parse and return the a3s-box skill, or `None` if the embedded markdown is malformed.
pub fn a3s_box_skill() -> Option<Arc<Skill>> {
    Skill::parse(A3S_BOX_SKILL_MD).map(Arc::new)
}

/// All SafeClaw built-in skills that are always available (unconditional).
///
/// Returns an empty vec for now — additional always-on SafeClaw skills can
/// be added here as the product evolves.
pub fn always_on_skills() -> Vec<Arc<Skill>> {
    vec![]
}
