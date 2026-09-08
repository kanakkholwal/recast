//! The words the model reads: the MCP `instructions` string and the installable skill, kept as documents so they can be edited as prose.

use std::path::{Path, PathBuf};

/// Returned from MCP `initialize`. Short by design; the skill carries the long form.
pub const MCP_INSTRUCTIONS: &str = include_str!("INSTRUCTIONS.md");

/// The skill file agents load on demand.
pub const SKILL: &str = include_str!("SKILL.md");

/// Directory name of the skill, which is also its `name:` in the frontmatter.
pub const SKILL_NAME: &str = "recast-editing";

/// Where `install_skill` writes when no directory is given: Claude Code's user skills.
pub fn default_skills_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".claude").join("skills"))
}

/// Writes `SKILL.md` under `<dir>/recast-editing/` and returns the file path.
/// # Errors When the directory cannot be created or the file cannot be written.
pub fn install_skill(dir: &Path) -> Result<PathBuf, String> {
    let target = dir.join(SKILL_NAME);
    std::fs::create_dir_all(&target).map_err(|e| format!("create {}: {e}", target.display()))?;
    let file = target.join("SKILL.md");
    std::fs::write(&file, SKILL).map_err(|e| format!("write {}: {e}", file.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::TOOLS;

    /// A tool the skill never mentions is one the model has to guess at.
    #[test]
    fn the_skill_names_every_tool() {
        for tool in TOOLS {
            assert!(
                SKILL.contains(tool.name),
                "SKILL.md does not mention {}",
                tool.name
            );
        }
    }

    #[test]
    fn the_skill_frontmatter_matches_its_directory_name() {
        assert!(SKILL.starts_with("---\nname: recast-editing\n"));
    }

    #[test]
    fn the_instructions_state_the_clock_rule_and_the_starting_tool() {
        assert!(MCP_INSTRUCTIONS.contains("OUTPUT seconds"));
        assert!(MCP_INSTRUCTIONS.contains("SOURCE seconds"));
        assert!(MCP_INSTRUCTIONS.contains("recast_project_list"));
    }

    #[test]
    fn install_writes_the_skill_where_asked() {
        let dir = std::env::temp_dir().join(format!("recast-skill-{}", std::process::id()));
        let file = install_skill(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), SKILL);
        assert!(file.ends_with(Path::new(SKILL_NAME).join("SKILL.md")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
