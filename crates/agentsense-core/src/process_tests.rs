// SPDX-License-Identifier: Apache-2.0
// Derived from Herdr; see the workspace NOTICE for provenance.

//! Upstream parser regressions, adapted to caller-supplied process inputs.

use super::*;
use crate::{
    full_lifecycle_hook_authority, interactive_agent_executable, parse_canonical_agent_label,
    session_identity_only_integration,
};

fn foreground_process(pid: u32, name: &str, argv: &[&str]) -> ForegroundProcess {
    ForegroundProcess {
        pid,
        name: name.to_string(),
        argv0: None,
        argv: Some(argv.iter().map(|arg| (*arg).to_string()).collect()),
        cmdline: Some(argv.join(" ")),
    }
}

// ---- Agent identification ----

#[test]
fn identify_known_agents() {
    assert_eq!(identify_agent("pi"), Some(Agent::Pi));
    assert_eq!(identify_agent("claude"), Some(Agent::Claude));
    assert_eq!(identify_agent("claude-code"), Some(Agent::Claude));
    assert_eq!(identify_agent("codex"), Some(Agent::Codex));
    assert_eq!(identify_agent("gemini"), Some(Agent::Gemini));
    assert_eq!(identify_agent("cursor"), Some(Agent::Cursor));
    assert_eq!(identify_agent("cursor-agent"), Some(Agent::Cursor));
    assert_eq!(identify_agent("devin"), Some(Agent::Devin));
    assert_eq!(identify_agent("devin-cli"), Some(Agent::Devin));
    assert_eq!(identify_agent("agy"), Some(Agent::Antigravity));
    assert_eq!(identify_agent("antigravity-cli"), Some(Agent::Antigravity));
    assert_eq!(identify_agent("cline"), Some(Agent::Cline));
    assert_eq!(identify_agent("omp"), Some(Agent::Omp));
    assert_eq!(identify_agent("mastracode"), Some(Agent::Mastracode));
    assert_eq!(identify_agent("mastra-code"), Some(Agent::Mastracode));
    assert_eq!(identify_agent("opencode"), Some(Agent::OpenCode));
    assert_eq!(identify_agent("opencode.exe"), Some(Agent::OpenCode));
    assert_eq!(identify_agent("opencode2"), Some(Agent::OpenCode));
    assert_eq!(identify_agent("opencode2.exe"), Some(Agent::OpenCode));
    assert_eq!(identify_agent("kimi"), Some(Agent::Kimi));
    assert_eq!(identify_agent("Kimi Code"), Some(Agent::Kimi));
    assert_eq!(identify_agent("kiro"), Some(Agent::Kiro));
    assert_eq!(identify_agent("kiro-cli"), Some(Agent::Kiro));
    assert_eq!(identify_agent("copilot"), Some(Agent::GithubCopilot));
    assert_eq!(identify_agent("ghcs"), Some(Agent::GithubCopilot));
    assert_eq!(identify_agent("grok"), Some(Agent::Grok));
    assert_eq!(identify_agent("grok-build"), Some(Agent::Grok));
    assert_eq!(identify_agent("hermes"), Some(Agent::Hermes));
    assert_eq!(identify_agent("hermes-agent"), Some(Agent::Hermes));
    assert_eq!(identify_agent("kilo"), Some(Agent::Kilo));
    assert_eq!(identify_agent("kilo-code"), Some(Agent::Kilo));
    assert_eq!(identify_agent("qwen"), Some(Agent::Qwen));
    assert_eq!(identify_agent("Qwen Code"), Some(Agent::Qwen));
    assert_eq!(identify_agent("letta"), Some(Agent::Letta));
    assert_eq!(identify_agent("Letta Code"), Some(Agent::Letta));
    assert_eq!(identify_agent("maki"), Some(Agent::Maki));
    assert_eq!(identify_agent("muse"), Some(Agent::Muse));
    assert_eq!(identify_agent("muse-code"), Some(Agent::Muse));
    assert_eq!(identify_agent("muse-cli"), Some(Agent::Muse));
    assert_eq!(identify_agent("muse-bin-0.1.0-R708.1"), Some(Agent::Muse));
    assert_eq!(identify_agent("muse-bin-1.2.3"), Some(Agent::Muse));
    assert_eq!(
        identify_agent("/home/user/.local/bin/muse-bin-0.2.1-R1215.1"),
        Some(Agent::Muse)
    );
    assert_eq!(
        identify_agent(r"C:\Users\user\muse-bin-0.2.1-R1215.1.exe"),
        Some(Agent::Muse)
    );
}

#[test]
fn parse_known_agent_labels() {
    assert_eq!(parse_agent_label("pi"), Some(Agent::Pi));
    assert_eq!(parse_agent_label("claude"), Some(Agent::Claude));
    assert_eq!(parse_agent_label("cursor-agent"), Some(Agent::Cursor));
    assert_eq!(parse_agent_label("devin-cli"), Some(Agent::Devin));
    assert_eq!(parse_agent_label("agy"), Some(Agent::Antigravity));
    assert_eq!(parse_agent_label("antigravity"), Some(Agent::Antigravity));
    assert_eq!(parse_agent_label("omp"), Some(Agent::Omp));
    assert_eq!(parse_agent_label("mastracode"), Some(Agent::Mastracode));
    assert_eq!(parse_agent_label("mastra code"), Some(Agent::Mastracode));
    assert_eq!(parse_agent_label("opencode.exe"), Some(Agent::OpenCode));
    assert_eq!(parse_agent_label("copilot"), Some(Agent::GithubCopilot));
    assert_eq!(parse_agent_label("kimi-code"), Some(Agent::Kimi));
    assert_eq!(
        parse_agent_label("github-copilot"),
        Some(Agent::GithubCopilot)
    );
    assert_eq!(parse_agent_label("amp-local"), Some(Agent::Amp));
    assert_eq!(parse_agent_label("kiro-cli"), Some(Agent::Kiro));
    assert_eq!(parse_agent_label("grok-build"), Some(Agent::Grok));
    assert_eq!(parse_agent_label("hermes-agent"), Some(Agent::Hermes));
    assert_eq!(parse_agent_label("qwen-code"), Some(Agent::Qwen));
    assert_eq!(parse_agent_label("letta-code"), Some(Agent::Letta));
    assert_eq!(parse_agent_label("maki"), Some(Agent::Maki));
    assert_eq!(parse_agent_label("kilo-code"), Some(Agent::Kilo));
}

#[test]
fn every_agent_label_round_trips_through_canonical_and_alias_parsers() {
    for agent in Agent::ALL {
        let label = agent_label(agent);
        assert_eq!(parse_canonical_agent_label(label), Some(agent));
        assert_eq!(parse_agent_label(label), Some(agent));
    }
}

#[test]
fn every_agent_has_a_canonical_interactive_executable() {
    let expected = [
        (Agent::Pi, "pi"),
        (Agent::Claude, "claude"),
        (Agent::Codex, "codex"),
        (Agent::Gemini, "gemini"),
        (
            Agent::Cursor,
            if cfg!(windows) {
                "cursor-agent.cmd"
            } else {
                "cursor-agent"
            },
        ),
        (Agent::Devin, "devin"),
        (Agent::Antigravity, "agy"),
        (Agent::Cline, "cline"),
        (Agent::Omp, "omp"),
        (Agent::Mastracode, "mastracode"),
        (Agent::OpenCode, "opencode"),
        (Agent::GithubCopilot, "copilot"),
        (Agent::Kimi, "kimi"),
        (Agent::Kiro, "kiro-cli"),
        (Agent::Droid, "droid"),
        (Agent::Amp, "amp"),
        (Agent::Grok, "grok"),
        (Agent::Hermes, "hermes"),
        (Agent::Kilo, "kilo"),
        (Agent::Qodercli, "qodercli"),
        (Agent::Qwen, "qwen"),
        (Agent::Letta, "letta"),
        (Agent::Maki, "maki"),
        (Agent::Muse, "muse"),
    ];
    assert_eq!(expected.len(), Agent::ALL.len());
    for (agent, executable) in expected {
        assert_eq!(interactive_agent_executable(agent), executable);
    }
}

#[test]
fn canonical_agent_labels_are_strict() {
    assert_eq!(parse_canonical_agent_label("claude-code"), None);
    assert_eq!(parse_canonical_agent_label("Pi"), None);
    assert_eq!(parse_canonical_agent_label(" pi "), None);
    assert_eq!(parse_canonical_agent_label("opencode.exe"), None);
}

#[test]
fn mastracode_is_hook_authority_without_screen_manifest() {
    assert!(full_lifecycle_hook_authority(
        "herdr:mastracode",
        "mastracode"
    ));
    assert!(!Agent::SCREEN_MANIFEST_AGENTS.contains(&Agent::Mastracode));
}

#[test]
fn session_identity_integrations_leave_state_to_screen_detection() {
    for (source, label, agent) in [
        ("herdr:hermes", "hermes", Agent::Hermes),
        ("herdr:qwen", "qwen", Agent::Qwen),
        ("herdr:letta", "letta", Agent::Letta),
        ("herdr:antigravity_cli", "agy", Agent::Antigravity),
    ] {
        assert!(!full_lifecycle_hook_authority(source, label));
        assert!(session_identity_only_integration(source, label));
        assert!(Agent::SCREEN_MANIFEST_AGENTS.contains(&agent));
    }
}

#[test]
fn identify_unknown_processes() {
    assert_eq!(identify_agent("bash"), None);
    assert_eq!(identify_agent("zsh"), None);
    assert_eq!(identify_agent("vim"), None);
    assert_eq!(identify_agent("node"), None);
    assert_eq!(identify_agent("museum"), None);
    assert_eq!(identify_agent("muse-helper"), None);
    assert_eq!(identify_agent("muser"), None);
    assert_eq!(identify_agent("musescore"), None);
    assert_eq!(identify_agent("muse-bin"), None);
    assert_eq!(identify_agent("muse-bin-"), None);
    assert_eq!(identify_agent("muse-binary"), None);
}

#[test]
fn identify_case_insensitive() {
    assert_eq!(identify_agent("Pi"), Some(Agent::Pi));
    assert_eq!(identify_agent("CLAUDE"), Some(Agent::Claude));
    assert_eq!(identify_agent("Codex"), Some(Agent::Codex));
    assert_eq!(identify_agent("Devin"), Some(Agent::Devin));
}

#[test]
fn identify_agent_in_job_prefers_wrapped_codex() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![
            foreground_process(1, "node", &["node", "/path/to/bin/codex"]),
            foreground_process(2, "bash", &["bash"]),
        ],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_node_wrapped_qwen() {
    for argv in [
        vec!["node", "/home/user/.fnm/bin/qwen"],
        vec![
            "node.exe",
            r"C:\Users\user\AppData\Roaming\npm\node_modules\@qwen-code\qwen-code\dist\index.js",
        ],
    ] {
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, "MainThread", &argv)],
        };

        assert_eq!(
            identify_agent_in_job(&job),
            Some((Agent::Qwen, "qwen".to_string()))
        );
    }
}

#[test]
fn identify_agent_in_job_detects_cline_native_binaries() {
    for (name, executable) in [
        (
            ".cline",
            "/home/user/.npm/lib/node_modules/cline/bin/.cline",
        ),
        (
            "cline",
            "/usr/local/lib/node_modules/@cline/cli-darwin-arm64/bin/cline",
        ),
        (
            "cline.exe",
            r"C:\Users\user\AppData\Roaming\npm\node_modules\@cline\cli-windows-x64\bin\cline.exe",
        ),
    ] {
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, name, &[executable, "--tui"])],
        };

        assert_eq!(
            identify_agent_in_job(&job),
            Some((Agent::Cline, name.to_string()))
        );
    }
}

#[test]
fn identify_agent_in_job_detects_cline_node_wrapper() {
    for (name, argv) in [
        (
            "MainThread",
            vec!["node", "/home/user/.fnm/bin/cline", "--tui"],
        ),
        (
            "node",
            vec!["node", "/usr/local/lib/node_modules/cline/bin/cline"],
        ),
        (
            "node.exe",
            vec![
                r"C:\Program Files\nodejs\node.exe",
                r"C:\Users\user\AppData\Roaming\npm\node_modules\cline\bin\cline",
            ],
        ),
    ] {
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, name, &argv)],
        };

        assert_eq!(
            identify_agent_in_job(&job),
            Some((Agent::Cline, "cline".to_string()))
        );
    }
}

#[test]
fn identify_agent_in_job_rejects_unrelated_cline_mentions() {
    for argv in [
        vec!["node"],
        vec!["node", "/path/to/other.js", "cline"],
        vec!["node", "-e", "cline"],
        vec!["node", "/path/to/cline-helper"],
        vec!["/path/to/.cline-helper"],
        vec!["/path/to/other", "/path/to/cline"],
    ] {
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, "MainThread", &argv)],
        };

        assert_eq!(identify_agent_in_job(&job), None);
    }
    assert_eq!(identify_agent("MainThread"), None);
}

#[test]
fn identify_agent_in_job_detects_interactive_letta_entrypoints() {
    for argv in [
        vec!["letta", "--backend", "local"],
        vec![
            "node",
            "/home/user/project/node_modules/.bin/letta",
            "--conversation",
            "conversation-id",
        ],
        vec![
            "node.exe",
            r"C:\Users\user\AppData\Roaming\npm\node_modules\@letta-ai\letta-code\letta.js",
            "--agent",
            "agent-id",
        ],
    ] {
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, "MainThread", &argv)],
        };

        assert_eq!(
            identify_agent_in_job(&job),
            Some((Agent::Letta, "letta".to_string()))
        );
    }
}

#[test]
fn identify_agent_in_job_ignores_noninteractive_letta_processes() {
    for args in [
        vec!["--prompt", "hello"],
        vec!["--output-format", "json"],
        vec!["--input-format=stream-json"],
        vec!["--ephemeral"],
        vec!["--max-turns=1"],
        vec!["server"],
        vec!["--backend", "local", "server"],
        vec!["fix this bug"],
        vec!["agents", "list"],
        vec!["version"],
    ] {
        let mut argv = vec!["node", "/home/user/project/node_modules/.bin/letta"];
        argv.extend(args);
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, "MainThread", &argv)],
        };

        assert_eq!(identify_agent_in_job(&job), None, "argv: {argv:?}");
    }

    let unrelated = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node",
            &["node", "/tmp/server.js", "letta"],
        )],
    };
    assert_eq!(identify_agent_in_job(&unrelated), None);

    let source_checkout = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node",
            &["node", "/home/user/src/letta-code/letta/build.js"],
        )],
    };
    assert_eq!(identify_agent_in_job(&source_checkout), None);
}

#[test]
fn identify_agent_in_job_detects_windows_cursor_install() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node.exe",
            &[
                r"C:\Users\user\AppData\Local\cursor-agent\versions\2026.08.11-e8db854\node.exe",
                r"C:\Users\user\AppData\Local\cursor-agent\versions\2026.08.11-e8db854\index.js",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Cursor, "cursor".to_string()))
    );
}

#[test]
fn identify_agent_in_job_ignores_invalid_windows_cursor_install_paths() {
    for script in [
        r"C:\Users\user\AppData\Local\cursor-agent\versions\2026.08.11-e8db854\scripts\postinstall.js",
        r"C:\Users\user\AppData\Local\cursor-agent\versions\2026.08.11-e8db854\index",
        r"C:\Users\user\AppData\Local\cursor-agent\versions\2026.08.11-e8db854\index.exe",
    ] {
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(
                123,
                "node.exe",
                &[
                    r"C:\Users\user\AppData\Local\cursor-agent\versions\2026.08.11-e8db854\node.exe",
                    script,
                ],
            )],
        };

        assert_eq!(identify_agent_in_job(&job), None, "script: {script}");
    }

    let lookalike = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node.exe",
            &[
                r"C:\Program Files\nodejs\node.exe",
                r"C:\workspace\cursor-agent\versions\test\index.js",
            ],
        )],
    };
    assert_eq!(identify_agent_in_job(&lookalike), None);
}

#[test]
fn identify_agent_in_job_prefers_recognized_process_group_leader() {
    let job = ForegroundJob {
        process_group_id: 42,
        processes: vec![
            foreground_process(42, "claude", &["claude"]),
            foreground_process(43, "node", &["node", "/tmp/mcp/bin/codex"]),
        ],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Claude, "claude".to_string()))
    );
}

#[test]
fn identify_agent_in_job_falls_back_when_process_group_leader_is_unrecognized() {
    let job = ForegroundJob {
        process_group_id: 42,
        processes: vec![
            foreground_process(42, "bash", &["bash"]),
            foreground_process(43, "node", &["node", "/tmp/mcp/bin/codex"]),
        ],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_python_version_wrapped_hermes() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "python3.12",
            &[
                "/nix/store/example/bin/python3.12",
                "/nix/store/example/bin/hermes",
                "--resume",
                "session-id",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Hermes, "hermes".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_nix_wrapped_codex_from_cmdline_argv0() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            ".codex-wrapped",
            &["/etc/profiles/per-user/user/bin/codex", "--model", "gpt-5"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn identify_agent_in_job_canonicalizes_nix_wrapped_aliases_from_cmdline_argv0() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            ".claude-code-wrapped",
            &["/nix/store/example/bin/claude-code"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Claude, "claude".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_shell_wrapped_pi() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "sh",
            &["/bin/sh", "/tmp/test-bin/pi"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Pi, "pi".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_bun_wrapped_omp() {
    for (runtime, script) in [
        ("bun", "/home/can/.bun/bin/omp"),
        (
            "bun.exe",
            r"C:\Users\herdr\AppData\Roaming\npm\node_modules\@oh-my-pi\pi-coding-agent\dist\cli.js",
        ),
    ] {
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, runtime, &[runtime, script])],
        };
        assert_eq!(
            identify_agent_in_job(&job),
            Some((Agent::Omp, "omp".to_string())),
            "script: {script}"
        );
    }

    let other_script = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "bun.exe",
            &[
                "bun.exe",
                r"C:\Users\herdr\AppData\Roaming\npm\node_modules\@oh-my-pi\pi-coding-agent\dist\setup.js",
            ],
        )],
    };
    assert_eq!(identify_agent_in_job(&other_script), None);
}

#[test]
fn identify_agent_in_job_detects_node_wrapped_pi_package_cli() {
    let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(
                123,
                "node.exe",
                &[
                    "node.exe",
                    "C:\\Users\\herdr\\AppData\\Roaming\\npm\\node_modules\\@earendil-works\\pi-coding-agent\\dist\\cli.js",
                ],
            )],
        };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Pi, "pi".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_node_wrapped_pi_bundled_cli() {
    // Hardened runtimes deny `PROCESS_VM_READ`, so the command line can come
    // from a LimitedInformation query with the launcher path intact. The
    // detection path must not depend on how that command line was obtained.
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node.exe",
            &[
                r"C:\Users\herdr\AppData\Local\pi-node\current\node.exe",
                r"C:\Users\herdr\AppData\Local\pi-node\current/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Pi, "pi".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_node_wrapped_mastracode_package_cli() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node.exe",
            &[
                "node.exe",
                "C:\\Users\\herdr\\AppData\\Roaming\\npm\\node_modules\\mastracode\\dist\\cli.js",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Mastracode, "mastracode".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_node_wrapped_kimi_package_cli() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "node.exe",
            &[
                r"C:\Program Files\nodejs\node.exe",
                r"C:\repro-3317-kimi-prefix\node_modules\@moonshot-ai\kimi-code\dist\main.mjs",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Kimi, "kimi".to_string()))
    );
}

#[test]
fn identify_agent_in_job_ignores_non_cli_pi_package_scripts() {
    for script in [
        r"C:\Users\herdr\AppData\Roaming\npm\node_modules\@earendil-works\pi-coding-agent\scripts\build.js",
        r"C:\Users\herdr\AppData\Local\pi-node\current\node_modules\@earendil-works\pi-coding-agent\dist\bundle\update.js",
        r"C:\workspace\dist\bundle\cli.js",
        r"C:\workspace\node_modules\other-package\dist\bundle\cli.js",
        r"C:\workspace\node_modules\@earendil-works\pi-coding-agent\dist\cli.exe",
        r"C:\workspace\node_modules\@earendil-works\pi-coding-agent\dist\cli.js\other.js",
        r"C:\workspace\node_modules\@earendil-works\pi-coding-agent\dist\bundle\cli.exe",
        r"C:\workspace\node_modules\@earendil-works\pi-coding-agent\dist\bundle\cli.js\other.js",
    ] {
        let job = ForegroundJob {
            process_group_id: 123,
            processes: vec![foreground_process(123, "node.exe", &["node.exe", script])],
        };

        assert_eq!(identify_agent_in_job(&job), None, "script: {script}");
    }
}

#[test]
fn identify_agent_in_job_detects_windows_cmd_wrapped_codex() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "cmd.exe",
            &[
                "cmd.exe",
                "/D",
                "/S",
                "/C",
                "C:\\Users\\herdr\\AppData\\Roaming\\npm\\codex.cmd --model gpt-5",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_powershell_file_wrapped_claude() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "powershell.exe",
            &[
                "powershell.exe",
                "-NoProfile",
                "-File",
                "C:\\Users\\herdr\\Documents\\PowerShell\\Scripts\\claude.ps1",
            ],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Claude, "claude".to_string()))
    );
}

// A plain shell pane launched with herdr's injected prompt integration
// must still classify as a shell, not an agent, even though its argv now
// carries a -Command payload.
#[test]
fn identify_agent_in_job_ignores_herdr_powershell_shell_integration_argv() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "powershell.exe",
            &[
                "powershell.exe",
                "-NoExit",
                "-Command",
                r"if ($null -eq $global:__HerdrOriginalPrompt) { $global:__HerdrOriginalPrompt = $function:prompt; function global:prompt { $out = @(& $global:__HerdrOriginalPrompt) -join ' '; $loc = $ExecutionContext.SessionState.Path.CurrentLocation; if ($loc.Provider.Name -eq 'FileSystem') { try { [Environment]::CurrentDirectory = $loc.ProviderPath } catch {}; $esc = [string][char]27; $out += $esc + ']9;9;' + $loc.ProviderPath + $esc + '\' }; $out } }",
            ],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn identify_agent_in_job_detects_opencode2_as_opencode() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "opencode2",
            &["opencode2", "--standalone"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::OpenCode, "opencode2".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_opencode_exe_from_pnpm_package() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "opencode.exe",
            &["/home/user/.local/share/pnpm/global/node_modules/opencode-ai/bin/opencode.exe"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::OpenCode, "opencode.exe".to_string()))
    );
}

#[test]
fn identify_agent_in_job_detects_opencode_exe_from_argv0_path() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            123,
            "MainThread",
            &["/home/user/.local/share/pnpm/global/node_modules/opencode-ai/bin/opencode.exe"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::OpenCode, "opencode".to_string()))
    );
}

#[test]
fn wrapped_agent_name_from_runtime_argv_ignores_plain_shell_flags() {
    assert_eq!(
        ProcessParser {
            resolve: |_: &str| None
        }
        .wrapped_agent_name_from_runtime_argv("bash", Some(&["bash".into(), "-lc".into()])),
        None
    );
}

#[test]
fn identify_agent_in_job_ignores_python_c_argument_named_codex() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "python3",
            &["python3", "-c", "import time; time.sleep(60)", "/tmp/codex"],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn identify_agent_in_job_ignores_node_eval_argument_named_codex() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "node",
            &["node", "-e", "setTimeout(() => {}, 60000)", "/tmp/codex"],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn identify_agent_in_job_ignores_shell_c_argument_named_codex() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "bash",
            &["bash", "-c", "sleep 60", "/tmp/codex"],
        )],
    };

    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn identify_agent_in_job_detects_python_script_named_codex() {
    let job = ForegroundJob {
        process_group_id: 123,
        processes: vec![foreground_process(
            1,
            "python3",
            &["python3", "/tmp/codex", "--model", "gpt-5"],
        )],
    };

    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".to_string()))
    );
}

#[test]
fn cmdline_argv0_agent_name_canonicalizes_known_aliases() {
    assert_eq!(
        ProcessParser {
            resolve: |_: &str| None
        }
        .cmdline_argv0_agent_name("/nix/store/example/bin/ghcs"),
        Some("copilot".to_string())
    );
}

#[test]
fn cmdline_argv0_agent_name_requires_exact_agent_basename() {
    assert_eq!(
        ProcessParser {
            resolve: |_: &str| None
        }
        .cmdline_argv0_agent_name("/tmp/my-codex-helper"),
        None
    );
}

#[test]
fn identify_agent_in_job_resolves_cursor_agent_symlink_argv0() {
    let job = ForegroundJob {
        process_group_id: 42,
        processes: vec![foreground_process(
            42,
            "MainThread",
            &["/tmp/agent", "--use-system-ca", "/tmp/index.js"],
        )],
    };
    assert_eq!(identify_agent_in_job(&job), None);
    assert_eq!(
        identify_agent_in_job_with_resolver(&job, |path| {
            (path == "/tmp/agent").then(|| "/opt/cursor-agent".to_owned())
        }),
        Some((Agent::Cursor, "cursor".to_owned())),
    );
}

#[test]
fn resolver_reaches_scripts_inside_runtime_wrappers() {
    let job = ForegroundJob {
        process_group_id: 7,
        processes: vec![foreground_process(7, "node", &["node", "/tmp/agent.js"])],
    };
    assert_eq!(identify_agent_in_job(&job), None);
    assert_eq!(
        identify_agent_in_job_with_resolver(&job, |path| {
            (path == "/tmp/agent.js").then(|| "/opt/codex.js".to_owned())
        }),
        Some((Agent::Codex, "codex".to_owned())),
    );
}

#[test]
fn resolved_letta_still_excludes_noninteractive_invocations() {
    let job = ForegroundJob {
        process_group_id: 7,
        processes: vec![foreground_process(
            7,
            "node",
            &["node", "/tmp/agent.js", "--print"],
        )],
    };
    assert_eq!(
        identify_agent_in_job_with_resolver(&job, |path| {
            (path == "/tmp/agent.js").then(|| "/opt/letta.js".to_owned())
        }),
        None,
    );
}

#[test]
fn environment_hints_accept_known_agents() {
    assert_eq!(
        parse_agent_env_hint(b"PATH=/bin\0HERDR_AGENT=claude\0TERM=xterm\0"),
        Some(Agent::Claude)
    );
    assert_eq!(
        parse_agent_env_hint(b"HERDR_AGENT=codex"),
        Some(Agent::Codex)
    );
}

#[test]
fn environment_hints_ignore_missing_unknown_or_invalid_values() {
    assert_eq!(parse_agent_env_hint(b"PATH=/bin\0TERM=xterm\0"), None);
    assert_eq!(parse_agent_env_hint(b"HERDR_AGENT=not-an-agent\0"), None);
    assert_eq!(parse_agent_env_hint(b"HERDR_AGENT=\xff\0"), None);
    assert_eq!(parse_agent_env_hint(b"NOT_HERDR_AGENT=claude\0"), None);
}
