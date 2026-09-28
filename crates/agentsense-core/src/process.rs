// SPDX-License-Identifier: Apache-2.0
// Derived from Herdr; see the workspace NOTICE for provenance.

//! Pure process-command parsing, including interpreter and shell wrappers.
//! Hosts supply process snapshots and any optional executable-path resolution.

use crate::registry::{normalized_agent_lookup_name, path_basename};
use crate::{agent_label, parse_agent_label, Agent, ForegroundJob, ForegroundProcess};

/// Identify an agent from its executable name or known label.
pub fn identify_agent(process_name: &str) -> Option<Agent> {
    parse_agent_label(process_name)
}

/// Identify the agent in a host-supplied foreground job.
///
/// This is a pure parser: executable symlinks are not resolved. Use
/// [`identify_agent_in_job_with_resolver`] when the host has resolved paths.
pub fn identify_agent_in_job(job: &ForegroundJob) -> Option<(Agent, String)> {
    identify_agent_in_job_with_resolver(job, |_| None)
}

/// Identify an agent while consulting a host-supplied path resolver.
///
/// `resolve` receives otherwise unrecognized path tokens, including scripts
/// run through interpreters or shells. It should return an absolute resolved
/// path, or `None` when no resolution is available. A host may use a cached map
/// or filesystem canonicalization; this crate itself performs no I/O.
pub fn identify_agent_in_job_with_resolver(
    job: &ForegroundJob,
    resolve: impl Fn(&str) -> Option<String>,
) -> Option<(Agent, String)> {
    ProcessParser { resolve }.identify_agent_in_job(job)
}

/// Read Herdr's `HERDR_AGENT` identity hint from a NUL-delimited environment.
/// Unknown or invalid UTF-8 hints return `None`; no environment is read directly.
pub fn parse_agent_env_hint(environ: &[u8]) -> Option<Agent> {
    for record in environ.split(|&byte| byte == 0) {
        let Some(value) = record.strip_prefix(b"HERDR_AGENT=") else {
            continue;
        };
        return parse_agent_label(std::str::from_utf8(value).ok()?);
    }
    None
}

struct ProcessParser<F> {
    resolve: F,
}

impl<F: Fn(&str) -> Option<String>> ProcessParser<F> {
    fn identify_agent_in_job(&self, job: &ForegroundJob) -> Option<(Agent, String)> {
        if let Some(process) = job
            .processes
            .iter()
            .find(|process| process.pid == job.process_group_id)
        {
            let candidate = self.normalized_process_name(process);
            if let Some(agent) = identify_agent(&candidate) {
                if agent != Agent::Letta || self.is_interactive_letta_process(process) {
                    return Some((agent, candidate));
                }
            }
        }

        let mut best: Option<(u8, Agent, String)> = None;

        for process in &job.processes {
            let candidate = self.normalized_process_name(process);
            let Some(agent) = identify_agent(&candidate) else {
                continue;
            };
            if agent == Agent::Letta && !self.is_interactive_letta_process(process) {
                continue;
            }
            let score = self.process_priority(process, &candidate);

            match &best {
                Some((best_score, _, _)) if *best_score >= score => {}
                _ => best = Some((score, agent, candidate)),
            }
        }

        best.map(|(_, agent, name)| (agent, name))
    }

    fn normalized_process_name(&self, process: &ForegroundProcess) -> String {
        let effective = process.argv0.as_deref().unwrap_or(&process.name);
        let lower_effective = effective.to_lowercase();

        if self.is_generic_runtime_or_shell(&lower_effective) {
            if let Some(wrapped_agent) =
                self.wrapped_agent_name_from_runtime_argv(&lower_effective, process.argv.as_deref())
            {
                return wrapped_agent;
            }
        }

        if identify_agent(effective).is_some() {
            return effective.to_string();
        }

        if let Some(runtime) = process.argv.as_deref().and_then(|argv| argv.first()) {
            let runtime_name = normalized_agent_lookup_name(path_basename(runtime));
            if matches!(runtime_name.as_str(), "node" | "bun") {
                if let Some(wrapped_agent) =
                    self.wrapped_agent_name_from_runtime_argv(runtime, process.argv.as_deref())
                {
                    if matches!(
                        identify_agent(&wrapped_agent),
                        Some(Agent::Qwen | Agent::Cline | Agent::Letta)
                    ) {
                        return wrapped_agent;
                    }
                }
            }
        }

        if let Some(wrapped_agent) = self.argv0_agent_name(process.argv.as_deref()).or_else(|| {
            self.cmdline_argv0_agent_name(process.cmdline.as_deref().unwrap_or_default())
        }) {
            return wrapped_agent;
        }

        effective.to_string()
    }

    fn wrapped_agent_name_from_runtime_argv(
        &self,
        runtime: &str,
        argv: Option<&[String]>,
    ) -> Option<String> {
        let argv = argv?;
        let runtime_name = normalized_agent_lookup_name(path_basename(runtime));

        match runtime_name.as_str() {
            "node" => self
                .cursor_agent_name_from_bundled_node_argv(argv)
                .or_else(|| {
                    self.script_arg_agent_name(argv, &["-e", "--eval", "-p", "--print"], &[])
                }),
            "bun" => self.script_arg_agent_name(argv, &["-e", "--eval", "-p", "--print"], &[]),
            name if self.is_python_runtime(name) => {
                self.script_arg_agent_name(argv, &["-c"], &["-m"])
            }
            "sh" | "bash" | "zsh" | "fish" => self.script_arg_agent_name(argv, &["-c"], &[]),
            "cmd" => self.windows_cmd_arg_agent_name(argv),
            "powershell" | "pwsh" => self.powershell_arg_agent_name(argv),
            "tmux" => None,
            _ => None,
        }
    }

    fn cursor_agent_name_from_bundled_node_argv(&self, argv: &[String]) -> Option<String> {
        let (runtime_parent, runtime_name) = self.path_parent_and_basename(argv.first()?)?;
        let (script_parent, script_name) = self.path_parent_and_basename(argv.get(1)?)?;
        if !runtime_name.eq_ignore_ascii_case("node.exe")
            || !script_name.eq_ignore_ascii_case("index.js")
            || !runtime_parent.eq_ignore_ascii_case(script_parent)
        {
            return None;
        }

        let mut tail = runtime_parent
            .rsplit(['/', '\\'])
            .filter(|component| !component.is_empty());
        let (Some(version), Some(versions), Some(package)) =
            (tail.next(), tail.next(), tail.next())
        else {
            return None;
        };
        (package.eq_ignore_ascii_case("cursor-agent")
            && versions.eq_ignore_ascii_case("versions")
            && !version.trim().is_empty())
        .then(|| agent_label(Agent::Cursor).to_string())
    }

    fn path_parent_and_basename<'a>(&self, path: &'a str) -> Option<(&'a str, &'a str)> {
        let split = path.rfind(['/', '\\'])?;
        let parent = path[..split].trim_end_matches(['/', '\\']);
        let basename = &path[split + 1..];
        (!parent.is_empty() && !basename.is_empty()).then_some((parent, basename))
    }

    fn windows_cmd_arg_agent_name(&self, argv: &[String]) -> Option<String> {
        let mut args = argv.iter().skip(1);
        while let Some(arg) = args.next() {
            let flag = arg.trim_matches('"').to_lowercase();
            match flag.as_str() {
                "/c" | "/k" => {
                    return args
                        .next()
                        .and_then(|command| self.command_text_agent_name(command))
                }
                "/d" | "/s" | "/q" | "/a" | "/u" | "/e:on" | "/e:off" | "/f:on" | "/f:off"
                | "/v:on" | "/v:off" => continue,
                _ => {}
            }
        }
        None
    }

    fn powershell_arg_agent_name(&self, argv: &[String]) -> Option<String> {
        let mut args = argv.iter().skip(1);
        while let Some(arg) = args.next() {
            let flag = arg.trim_matches('"').to_lowercase();
            match flag.as_str() {
                "-file" | "-f" | "/file" => {
                    return args
                        .next()
                        .and_then(|path| self.agent_name_from_path_token(path));
                }
                "-command" | "-c" | "/command" | "/c" => {
                    return args
                        .next()
                        .and_then(|command| self.command_text_agent_name(command));
                }
                "-encodedcommand" | "-enc" | "/encodedcommand" | "/enc" => return None,
                "-configurationname" | "-executionpolicy" | "-outputformat" | "-psconsolefile"
                | "-version" | "-windowstyle" | "-workingdirectory" => {
                    let _ = args.next();
                }
                _ if flag.starts_with('-') || flag.starts_with('/') => {}
                _ => return self.agent_name_from_path_token(arg),
            }
        }
        None
    }

    fn command_text_agent_name(&self, command: &str) -> Option<String> {
        let mut rest = command;
        while let Some((token, next)) = self.command_text_token(rest) {
            let token = token.trim();
            if token.eq_ignore_ascii_case("&")
                || token.eq_ignore_ascii_case(".")
                || token.eq_ignore_ascii_case("call")
            {
                rest = next;
                continue;
            }
            return self.agent_name_from_path_token(token);
        }
        None
    }

    fn command_text_token<'a>(&self, input: &'a str) -> Option<(&'a str, &'a str)> {
        let input = input.trim_start();
        let first = input.chars().next()?;
        if first == '"' || first == '\'' {
            let start = first.len_utf8();
            if let Some(end) = input[start..].find(first) {
                let end = start + end;
                return Some((&input[start..end], &input[end + first.len_utf8()..]));
            }
            return Some((&input[start..], ""));
        }

        let end = input.find(char::is_whitespace).unwrap_or(input.len());
        Some((&input[..end], &input[end..]))
    }

    fn script_arg_agent_name(
        &self,
        argv: &[String],
        eval_flags: &[&str],
        module_flags: &[&str],
    ) -> Option<String> {
        let mut args = argv.iter().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--" {
                return args
                    .next()
                    .and_then(|token| self.agent_name_from_path_token(token));
            }

            if self.flag_matches(arg, eval_flags) || self.flag_matches(arg, module_flags) {
                return None;
            }

            if arg.starts_with('-') {
                if self.option_takes_value(arg) {
                    let _ = args.next();
                }
                continue;
            }

            return self.agent_name_from_path_token(arg);
        }

        None
    }

    fn flag_matches(&self, arg: &str, flags: &[&str]) -> bool {
        flags.iter().any(|flag| {
            arg == *flag || self.short_flag_payload(arg, flag) || self.long_flag_value(arg, flag)
        })
    }

    fn short_flag_payload(&self, arg: &str, flag: &str) -> bool {
        flag.starts_with('-')
            && !flag.starts_with("--")
            && arg.starts_with(flag)
            && arg.len() > flag.len()
    }

    fn long_flag_value(&self, arg: &str, flag: &str) -> bool {
        flag.starts_with("--")
            && arg
                .strip_prefix(flag)
                .is_some_and(|rest| rest.starts_with('='))
    }

    fn option_takes_value(&self, arg: &str) -> bool {
        matches!(
            arg,
            "-r" | "--require"
                | "--loader"
                | "--import"
                | "--experimental-loader"
                | "--inspect-port"
                | "-W"
                | "-X"
                | "-S"
                | "-L"
                | "-o"
        )
    }

    fn argv0_agent_name(&self, argv: Option<&[String]>) -> Option<String> {
        self.agent_name_from_path_token(argv?.first()?)
    }

    fn cmdline_argv0_agent_name(&self, cmdline: &str) -> Option<String> {
        self.agent_name_from_path_token(cmdline.split_whitespace().next()?)
    }

    fn agent_name_from_path_token(&self, token: &str) -> Option<String> {
        let trimmed = token.trim_matches(|c| matches!(c, '"' | '\''));
        if trimmed.is_empty() || trimmed.starts_with('-') {
            return None;
        }

        self.agent_name_from_basename(path_basename(trimmed))
            .or_else(|| self.agent_name_from_known_package_path(trimmed))
            .or_else(|| self.resolved_agent_name_from_path_token(trimmed))
    }

    fn agent_name_from_known_package_path(&self, path: &str) -> Option<String> {
        let raw_components: Vec<&str> = path
            .split(['/', '\\'])
            .filter(|component| !component.is_empty())
            .collect();
        let ends_with = |suffix: &[&str]| {
            raw_components.len() >= suffix.len()
                && raw_components[raw_components.len() - suffix.len()..]
                    .iter()
                    .zip(suffix)
                    .all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
        };
        if ends_with(&[
            "node_modules",
            "@earendil-works",
            "pi-coding-agent",
            "dist",
            "cli.js",
        ]) || ends_with(&[
            "node_modules",
            "@earendil-works",
            "pi-coding-agent",
            "dist",
            "bundle",
            "cli.js",
        ]) {
            return Some(agent_label(Agent::Pi).to_string());
        }
        if ends_with(&[
            "node_modules",
            "@oh-my-pi",
            "pi-coding-agent",
            "dist",
            "cli.js",
        ]) {
            return Some(agent_label(Agent::Omp).to_string());
        }
        if ends_with(&[
            "node_modules",
            "@moonshot-ai",
            "kimi-code",
            "dist",
            "main.mjs",
        ]) {
            return Some(agent_label(Agent::Kimi).to_string());
        }

        let components: Vec<String> = raw_components
            .into_iter()
            .map(normalized_agent_lookup_name)
            .collect();
        for window in components.windows(5) {
            if window == ["node_modules", "@qwen-code", "qwen-code", "dist", "index"] {
                return Some(agent_label(Agent::Qwen).to_string());
            }
        }
        for window in components.windows(4) {
            if window == ["node_modules", "mastracode", "dist", "cli"] {
                return Some(agent_label(Agent::Mastracode).to_string());
            }
            if window == ["node_modules", "@letta-ai", "letta-code", "letta"] {
                return Some(agent_label(Agent::Letta).to_string());
            }
        }
        None
    }

    fn letta_entrypoint_index(&self, argv: &[String]) -> Option<usize> {
        let is_letta = |arg: &str| {
            self.agent_name_from_path_token(arg).as_deref() == Some(agent_label(Agent::Letta))
        };
        if argv.first().is_some_and(|arg| is_letta(arg)) {
            return Some(0);
        }

        let runtime = argv
            .first()
            .map(|arg| normalized_agent_lookup_name(path_basename(arg)))?;
        if !matches!(runtime.as_str(), "node" | "bun") {
            return None;
        }

        let mut index = 1;
        while let Some(arg) = argv.get(index) {
            if arg == "--" {
                return argv
                    .get(index + 1)
                    .is_some_and(|arg| is_letta(arg))
                    .then_some(index + 1);
            }
            if self.flag_matches(arg, &["-e", "--eval", "-p", "--print"]) {
                return None;
            }
            if arg.starts_with('-') {
                index += if self.option_takes_value(arg) { 2 } else { 1 };
                continue;
            }
            return is_letta(arg).then_some(index);
        }
        None
    }

    fn letta_first_arg_after_backend_selection<'a>(&self, args: &'a [String]) -> Option<&'a str> {
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if arg == "--backend" {
                let _ = args.next();
                continue;
            }
            if arg.starts_with("--backend=") {
                continue;
            }
            return Some(arg);
        }
        None
    }

    fn is_interactive_letta_process(&self, process: &ForegroundProcess) -> bool {
        let parsed_cmdline;
        let argv = if let Some(argv) = process.argv.as_deref() {
            argv
        } else {
            parsed_cmdline = process
                .cmdline
                .as_deref()
                .unwrap_or_default()
                .split_whitespace()
                .map(|arg| arg.trim_matches(|ch| matches!(ch, '\'' | '"')).to_string())
                .collect::<Vec<_>>();
            if parsed_cmdline.is_empty() {
                return true;
            }
            &parsed_cmdline
        };

        let cli_args = self
            .letta_entrypoint_index(argv)
            .map(|index| &argv[index + 1..])
            .unwrap_or(argv);

        if cli_args.iter().any(|arg| {
            let option = arg.split_once('=').map_or(arg.as_str(), |(name, _)| name);
            matches!(
                option,
                "-p" | "--print"
                    | "--prompt"
                    | "--json"
                    | "--stream-json"
                    | "--run"
                    | "--disable-memory-guard"
                    | "--output-format"
                    | "--input-format"
                    | "--include-partial-messages"
                    | "--from-agent"
                    | "--environment"
                    | "--env"
                    | "--pre-load-skills"
                    | "--tags"
                    | "--ephemeral"
                    | "--stateless"
                    | "--max-turns"
                    | "--memfs-startup"
                    | "-h"
                    | "--help"
                    | "-v"
                    | "--version"
                    | "--info"
                    | "--update"
                    | "--upgrade"
            )
        }) {
            return false;
        }

        self.letta_first_arg_after_backend_selection(cli_args)
            .is_none_or(|arg| arg.starts_with('-'))
    }

    fn resolved_agent_name_from_path_token(&self, token: &str) -> Option<String> {
        if !token.contains(['/', '\\']) {
            return None;
        }
        let resolved = (self.resolve)(token)?;
        self.agent_name_from_basename(path_basename(&resolved))
    }

    fn agent_name_from_basename(&self, basename: &str) -> Option<String> {
        let agent = parse_agent_label(basename)?;
        Some(agent_label(agent).to_string())
    }

    fn process_priority(&self, process: &ForegroundProcess, normalized_name: &str) -> u8 {
        let lower_name = normalized_name.to_lowercase();
        if lower_name != process.name.to_lowercase() {
            return 3;
        }
        if !self.is_generic_runtime_or_shell(&lower_name) {
            return 2;
        }
        1
    }

    fn is_generic_runtime_or_shell(&self, name: &str) -> bool {
        let name = normalized_agent_lookup_name(path_basename(name));
        self.is_python_runtime(&name)
            || matches!(
                name.as_str(),
                "sh" | "bash"
                    | "zsh"
                    | "fish"
                    | "tmux"
                    | "node"
                    | "bun"
                    | "cmd"
                    | "powershell"
                    | "pwsh"
            )
    }

    fn is_python_runtime(&self, name: &str) -> bool {
        name == "python"
            || name.strip_prefix("python").is_some_and(|version| {
                !version.is_empty()
                    && version
                        .split('.')
                        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
            })
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
