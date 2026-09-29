// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr; see the workspace NOTICE.

//! Agent names and process recognition. The caller supplies foreground process data.

use crate::Agent;

/// Canonical lowercase label for an agent.
pub fn agent_label(agent: Agent) -> &'static str {
    match agent {
        Agent::Pi => "pi",
        Agent::Claude => "claude",
        Agent::Codex => "codex",
        Agent::Gemini => "gemini",
        Agent::Cursor => "cursor",
        Agent::Devin => "devin",
        Agent::Antigravity => "agy",
        Agent::Cline => "cline",
        Agent::Omp => "omp",
        Agent::Mastracode => "mastracode",
        Agent::OpenCode => "opencode",
        Agent::GithubCopilot => "copilot",
        Agent::Kimi => "kimi",
        Agent::Kiro => "kiro",
        Agent::Droid => "droid",
        Agent::Amp => "amp",
        Agent::Grok => "grok",
        Agent::Hermes => "hermes",
        Agent::Kilo => "kilo",
        Agent::Qodercli => "qodercli",
        Agent::Qwen => "qwen",
        Agent::Letta => "letta",
        Agent::Maki => "maki",
        Agent::Muse => "muse",
    }
}

/// Recognize a canonical label, known alias, or executable basename.
pub fn parse_agent_label(value: &str) -> Option<Agent> {
    let basename = value
        .trim()
        .rsplit(['/', '\\'])
        .next()?
        .to_ascii_lowercase();
    let name = [".exe", ".cmd", ".bat", ".ps1", ".js"]
        .iter()
        .find_map(|suffix| basename.strip_suffix(suffix))
        .unwrap_or(&basename);
    match name {
        "pi" => Some(Agent::Pi),
        "claude" | "claude-code" => Some(Agent::Claude),
        "codex" => Some(Agent::Codex),
        "gemini" => Some(Agent::Gemini),
        "cursor" | "cursor-agent" => Some(Agent::Cursor),
        "devin" | "devin-cli" => Some(Agent::Devin),
        "agy" | "antigravity" | "antigravity-cli" => Some(Agent::Antigravity),
        "cline" | ".cline" => Some(Agent::Cline),
        "omp" => Some(Agent::Omp),
        "mastracode" | "mastra-code" => Some(Agent::Mastracode),
        "opencode" | "opencode2" | "open-code" => Some(Agent::OpenCode),
        "copilot" | "github-copilot" | "ghcs" => Some(Agent::GithubCopilot),
        "kimi" | "kimi-code" => Some(Agent::Kimi),
        "kiro" | "kiro-cli" => Some(Agent::Kiro),
        "droid" => Some(Agent::Droid),
        "amp" | "amp-local" => Some(Agent::Amp),
        "grok" | "grok-build" => Some(Agent::Grok),
        "hermes" | "hermes-agent" => Some(Agent::Hermes),
        "kilo" | "kilo-code" => Some(Agent::Kilo),
        "qodercli" | "qoderclicn" | "qoder" | "qodercn" => Some(Agent::Qodercli),
        "qwen" | "qwen-code" => Some(Agent::Qwen),
        "letta" | "letta-code" => Some(Agent::Letta),
        "maki" => Some(Agent::Maki),
        "muse" | "muse-code" | "muse-cli" => Some(Agent::Muse),
        _ if name
            .strip_prefix("muse-bin-")
            .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit())) =>
        {
            Some(Agent::Muse)
        }
        _ => None,
    }
}

/// Identify an agent in a foreground process observed by the terminal host.
///
/// Runtime wrappers are checked only for known runtimes. The command line is
/// split on whitespace because this is the form alacritree's probe provides;
/// hosts with argv boundaries may pass the executable or script name directly.
pub fn identify_process(process_name: Option<&str>, cmdline: Option<&str>) -> Option<Agent> {
    let process_name = process_name.unwrap_or("").trim();
    if let Some(agent) = parse_agent_label(process_name) {
        return Some(agent);
    }
    let runtime = process_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(process_name)
        .to_ascii_lowercase();
    if !matches!(
        runtime.as_str(),
        "node"
            | "node.exe"
            | "bun"
            | "python"
            | "python3"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "cmd"
            | "cmd.exe"
            | "powershell"
            | "pwsh"
    ) {
        return None;
    }
    cmdline?
        .split_whitespace()
        .skip(1)
        .find(|token| !token.starts_with('-'))
        .and_then(|token| parse_agent_label(token.trim_matches(['\'', '"'])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_round_trips() {
        for agent in Agent::ALL {
            assert_eq!(parse_agent_label(agent_label(agent)), Some(agent));
            assert_eq!(
                identify_process(Some(agent_label(agent)), None),
                Some(agent)
            );
        }
    }

    #[test]
    fn foreground_recognition_is_scoped_to_executable_and_runtime() {
        assert_eq!(identify_process(Some("codex"), None), Some(Agent::Codex));
        assert_eq!(
            identify_process(Some("node"), Some("node /tmp/claude.js")),
            Some(Agent::Claude)
        );
        assert_eq!(identify_process(Some("echo"), Some("echo claude")), None);
        assert_eq!(
            identify_process(Some("bash"), Some("bash -c echo claude")),
            None
        );
    }
}
