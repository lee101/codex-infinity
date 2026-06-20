use crate::MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT;
use codex_utils_absolute_path::AbsolutePathBuf;

pub async fn build_memory_tool_developer_instructions(
    codex_home: &AbsolutePathBuf,
) -> Option<String> {
    let base_path = codex_home.join("memories");
    let memory_summary_path = base_path.join("memory_summary.md");
    let mut memory_summary = std::fs::read_to_string(&memory_summary_path).ok()?;
    memory_summary = memory_summary.trim().to_string();
    if memory_summary.is_empty() {
        return None;
    }
    if memory_summary.len() > MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT * 4 {
        memory_summary.truncate(MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT * 4);
    }

    Some(format!(
        "Memory files are stored under `{}`.\n- {}/memory_summary.md (already provided below; do NOT open again)\n\n========= MEMORY_SUMMARY BEGINS =========\n{}\n========= MEMORY_SUMMARY ENDS =========",
        base_path.display(),
        base_path.display(),
        memory_summary
    ))
}
