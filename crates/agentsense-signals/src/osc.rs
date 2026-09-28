// SPDX-License-Identifier: Apache-2.0
// Derived from Herdr; see the workspace NOTICE for provenance.

//! Bounded, incremental capture of agent OSC title and progress evidence.
//!
//! Framing survives split PTY reads. Oversized sequences are discarded, and
//! DCS/APC/PM/SOS strings are ignored so their contents cannot spoof OSC evidence.

fn is_ignored_string_intro(byte: u8) -> bool {
    matches!(byte, b'P' | b'_' | b'^' | b'X')
}

/// Collects complete OSC bodies from a raw byte stream. Consumers receive only
/// bodies, keeping the framing state machine independent from OSC commands.
#[derive(Debug, Default)]
struct OscStreamCollector {
    state: OscStreamState,
    body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum OscStreamState {
    #[default]
    Ground,
    Escape,
    Body,
    BodyEscape,
    IgnoringString,
    IgnoringStringEscape,
    Discarding,
    DiscardingEscape,
}

impl OscStreamCollector {
    const MAX_BODY_BYTES: usize = 4096;

    fn observe(&mut self, bytes: &[u8], mut receive: impl FnMut(&[u8])) {
        let mut cursor = 0;
        while cursor < bytes.len() {
            if matches!(
                self.state,
                OscStreamState::Ground | OscStreamState::IgnoringString
            ) {
                let Some(offset) = bytes[cursor..].iter().position(|&byte| byte == 0x1b) else {
                    break;
                };
                cursor += offset;
            }
            let byte = bytes[cursor];
            cursor += 1;
            match self.state {
                OscStreamState::Ground => {
                    if byte == 0x1b {
                        self.state = OscStreamState::Escape;
                    }
                }
                OscStreamState::Escape => match byte {
                    b']' => {
                        self.body.clear();
                        self.state = OscStreamState::Body;
                    }
                    0x1b => self.state = OscStreamState::Escape,
                    byte if is_ignored_string_intro(byte) => {
                        self.state = OscStreamState::IgnoringString;
                    }
                    _ => self.state = OscStreamState::Ground,
                },
                OscStreamState::Body => match byte {
                    0x07 => self.finish(&mut receive),
                    0x1b => self.state = OscStreamState::BodyEscape,
                    _ => self.push(byte),
                },
                OscStreamState::BodyEscape => match byte {
                    b'\\' => self.finish(&mut receive),
                    0x07 => {
                        self.push(0x1b);
                        if matches!(self.state, OscStreamState::Body) {
                            self.finish(&mut receive);
                        } else {
                            self.state = OscStreamState::Ground;
                        }
                    }
                    0x1b => {
                        self.push(0x1b);
                        self.state = match self.state {
                            OscStreamState::Body => OscStreamState::BodyEscape,
                            OscStreamState::Discarding => OscStreamState::DiscardingEscape,
                            state => state,
                        };
                    }
                    _ => {
                        self.push(0x1b);
                        if matches!(self.state, OscStreamState::Body) {
                            self.push(byte);
                        }
                    }
                },
                OscStreamState::IgnoringString => {
                    if byte == 0x1b {
                        self.state = OscStreamState::IgnoringStringEscape;
                    }
                }
                OscStreamState::IgnoringStringEscape => {
                    if byte == b'\\' {
                        self.state = OscStreamState::Ground;
                    } else if byte != 0x1b {
                        self.state = OscStreamState::IgnoringString;
                    }
                }
                OscStreamState::Discarding => {
                    if byte == 0x07 {
                        self.state = OscStreamState::Ground;
                    } else if byte == 0x1b {
                        self.state = OscStreamState::DiscardingEscape;
                    }
                }
                OscStreamState::DiscardingEscape => {
                    if byte == b'\\' || byte == 0x07 {
                        self.state = OscStreamState::Ground;
                    } else if byte != 0x1b {
                        self.state = OscStreamState::Discarding;
                    }
                }
            }
        }
    }

    fn push(&mut self, byte: u8) {
        self.body.push(byte);
        if self.body.len() > Self::MAX_BODY_BYTES {
            self.body.clear();
            self.state = OscStreamState::Discarding;
        } else {
            self.state = OscStreamState::Body;
        }
    }

    fn finish(&mut self, receive: &mut impl FnMut(&[u8])) {
        receive(&self.body);
        self.body.clear();
        self.state = OscStreamState::Ground;
    }
}

/// Maximum retained string length for agent OSC title and progress payloads.
/// Title text is untrusted model output; cap it to bound memory and log size.
const AGENT_OSC_MAX_CHARS: usize = 256;

/// Always-on tracker that retains the latest OSC 0/2 title and OSC 9 progress
/// payload emitted by the child process. Nothing here affects rendering; this
/// is pure passive capture for the detection engine.
///
/// - `latest_title` — last OSC 0 or OSC 2 payload, sanitized. An empty
///   payload (e.g. `\x1b]0;\x07`) clears the stored value.
/// - `latest_progress` — last OSC 9 payload (the part after `9;`), stored
///   as-is after sanitization. E.g. `"4;3;"` or `"4;0;"`.
#[derive(Debug, Default)]
pub struct AgentOscStateTracker {
    collector: OscStreamCollector,
    latest_title: Option<String>,
    terminal_title: Option<String>,
    latest_progress: Option<String>,
}

impl AgentOscStateTracker {
    /// Consume the next stream chunk and report whether the terminal title changed.
    /// Progress changes are observable through [`Self::latest_progress`].
    pub fn observe(&mut self, bytes: &[u8]) -> bool {
        let (collector, latest_title, terminal_title, latest_progress) = (
            &mut self.collector,
            &mut self.latest_title,
            &mut self.terminal_title,
            &mut self.latest_progress,
        );
        let mut terminal_title_changed = false;
        collector.observe(bytes, |body| {
            let Some((command, payload)) = parse_agent_osc_body(body) else {
                return;
            };
            match command {
                b"0" | b"2" => {
                    let title = sanitize_agent_osc_string(payload, AGENT_OSC_MAX_CHARS);
                    let title = (!title.is_empty()).then_some(title);
                    terminal_title_changed |= *terminal_title != title;
                    *terminal_title = title.clone();
                    *latest_title = title;
                }
                b"9" => {
                    *latest_progress =
                        Some(sanitize_agent_osc_string(payload, AGENT_OSC_MAX_CHARS));
                }
                _ => {}
            }
        });
        terminal_title_changed
    }

    pub fn terminal_title(&self) -> Option<&str> {
        self.terminal_title.as_deref()
    }

    /// Restore a display title without restoring agent detection evidence.
    pub fn seed_terminal_title(&mut self, title: Option<String>) {
        self.terminal_title = title;
    }

    /// Returns the latest retained OSC title, or `""` if none has been seen or
    /// the last title was an empty clear.
    pub fn latest_title(&self) -> &str {
        self.latest_title.as_deref().unwrap_or("")
    }

    /// Returns the latest retained OSC 9 progress payload, or `""` if none.
    pub fn latest_progress(&self) -> &str {
        self.latest_progress.as_deref().unwrap_or("")
    }

    /// Drops the retained title and progress so a new foreground agent cannot
    /// inherit OSC evidence emitted by a previous process. The in-flight parse
    /// state is kept: a sequence spanning the agent change finalizes normally
    /// and is attributed to the new agent.
    pub fn clear_retained(&mut self) {
        self.latest_title = None;
        self.latest_progress = None;
    }
}

/// Splits an OSC body at the first `;`, returning `(command, payload)`.
/// Returns `None` if there is no `;`.
fn parse_agent_osc_body(body: &[u8]) -> Option<(&[u8], &[u8])> {
    let sep = body.iter().position(|&b| b == b';')?;
    Some((&body[..sep], &body[sep + 1..]))
}

fn sanitize_agent_osc_string(payload: &[u8], max_chars: usize) -> String {
    let text = String::from_utf8_lossy(payload);
    let mut out = String::new();
    for ch in text.chars().filter(|ch| !ch.is_control()).take(max_chars) {
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc_stream_collector_ignores_strings_and_preserves_escaped_bytes() {
        let mut collector = OscStreamCollector::default();
        let mut bodies = Vec::new();

        collector.observe(
            b"\x1bPignored\x1b]0;not-osc\x07\x1b\\\x1b]9;a\x1b",
            |body| bodies.push(body.to_vec()),
        );
        collector.observe(b"\x1b\\\x1b]2;b\x1b\x07", |body| bodies.push(body.to_vec()));

        assert_eq!(bodies, vec![b"9;a\x1b".to_vec(), b"2;b\x1b".to_vec()]);
    }

    #[test]
    fn oversized_osc_recovers_after_escape_bel_termination() {
        let mut tracker = AgentOscStateTracker::default();
        tracker.observe(b"\x1b]0;before\x07");
        let mut oversized = b"\x1b]0;".to_vec();
        oversized.extend(std::iter::repeat_n(b'x', 4097));
        oversized.push(0x1b);
        tracker.observe(&oversized);
        tracker.observe(b"\x07\x1b]0;after\x07");
        assert_eq!(tracker.latest_title(), "after");
    }

    #[test]
    fn ignores_embedded_osc_inside_other_terminal_strings() {
        for intro in *b"P_^X" {
            let mut tracker = AgentOscStateTracker::default();
            let mut stream = vec![0x1b, intro];
            stream.extend_from_slice(b"ignored\x1b]0;spoofed\x07\x1b\\");
            stream.extend_from_slice(b"\x1b]2;real\x07");
            for byte in stream {
                tracker.observe(&[byte]);
            }
            assert_eq!(tracker.latest_title(), "real");
        }
    }

    #[test]
    fn every_split_preserves_multibyte_title_and_progress() {
        let stream = "\x1b]2;修复🙂\x1b\\\x1b]9;4;3;\x07".as_bytes();
        for split in 0..=stream.len() {
            let mut tracker = AgentOscStateTracker::default();
            tracker.observe(&stream[..split]);
            tracker.observe(&stream[split..]);
            assert_eq!(tracker.latest_title(), "修复🙂");
            assert_eq!(tracker.latest_progress(), "4;3;");
        }
    }

    #[test]
    fn agent_osc_osc0_title_with_bel() {
        let mut t = AgentOscStateTracker::default();
        t.observe("hello\x1b]0;braille title\x07world".as_bytes());
        assert_eq!(t.latest_title(), "braille title");
        assert_eq!(t.terminal_title(), Some("braille title"));
        assert_eq!(t.latest_progress(), "");
    }

    #[test]
    fn agent_osc_osc2_title_with_st() {
        let mut t = AgentOscStateTracker::default();
        t.observe("hello\x1b]2;static title\x1b\\world".as_bytes());
        assert_eq!(t.latest_title(), "static title");
        assert_eq!(t.latest_progress(), "");
    }

    #[test]
    fn agent_osc_empty_osc0_clears_title() {
        let mut t = AgentOscStateTracker::default();
        // First set a title.
        t.observe(b"\x1b]0;some title\x07");
        assert_eq!(t.latest_title(), "some title");
        // Then clear it with an empty payload (Codex pattern).
        t.observe(b"\x1b]0;\x07");
        assert_eq!(t.latest_title(), "");
        assert_eq!(t.terminal_title(), None);
    }

    #[test]
    fn clearing_agent_evidence_preserves_the_terminal_title() {
        let mut tracker = AgentOscStateTracker::default();
        tracker.observe("\x1b]2;✳ 修复🙂标题\x1b\\".as_bytes());

        tracker.clear_retained();

        assert_eq!(tracker.latest_title(), "");
        assert_eq!(tracker.terminal_title(), Some("✳ 修复🙂标题"));
    }

    #[test]
    fn handoff_seed_does_not_restore_agent_detection_evidence() {
        let mut tracker = AgentOscStateTracker::default();

        tracker.seed_terminal_title(Some("✳ restored title".into()));

        assert_eq!(tracker.terminal_title(), Some("✳ restored title"));
        assert_eq!(tracker.latest_title(), "");
    }

    #[test]
    fn agent_osc_osc9_sets_progress_with_bel() {
        let mut t = AgentOscStateTracker::default();
        t.observe(b"\x1b]9;4;3;\x07");
        assert_eq!(t.latest_progress(), "4;3;");
        assert_eq!(t.latest_title(), "");
    }

    #[test]
    fn agent_osc_osc9_clear_progress_with_st() {
        let mut t = AgentOscStateTracker::default();
        t.observe(b"\x1b]9;4;3;\x07");
        assert_eq!(t.latest_progress(), "4;3;");
        t.observe(b"\x1b]9;4;0;\x1b\\");
        assert_eq!(t.latest_progress(), "4;0;");
    }

    #[test]
    fn agent_osc_split_sequence_across_chunks() {
        let mut t = AgentOscStateTracker::default();
        t.observe(b"\x1b]9;4;3");
        assert_eq!(t.latest_progress(), "");
        t.observe(b";\x07");
        assert_eq!(t.latest_progress(), "4;3;");
    }

    #[test]
    fn agent_osc_bel_and_st_terminators_both_work() {
        let mut t = AgentOscStateTracker::default();
        t.observe(b"\x1b]0;title-bel\x07");
        assert_eq!(t.latest_title(), "title-bel");
        t.observe(b"\x1b]0;title-st\x1b\\");
        assert_eq!(t.latest_title(), "title-st");
    }

    #[test]
    fn agent_osc_oversized_payload_is_discarded_and_recovers() {
        let mut t = AgentOscStateTracker::default();
        // Set a title first.
        t.observe(b"\x1b]0;before\x07");
        assert_eq!(t.latest_title(), "before");

        // Feed an oversized OSC body (> 4096 bytes).
        let mut oversized = Vec::from(b"\x1b]0;".as_slice());
        oversized.extend(std::iter::repeat_n(b'x', 4097));
        oversized.push(0x07);
        t.observe(&oversized);
        // The oversized body is dropped; the previously stored title is kept.
        assert_eq!(t.latest_title(), "before");

        // After recovery, subsequent valid sequences are captured normally.
        t.observe(b"\x1b]0;after\x07");
        assert_eq!(t.latest_title(), "after");
    }

    #[test]
    fn agent_osc_cap_length_is_respected() {
        let mut t = AgentOscStateTracker::default();
        // Build a title of AGENT_OSC_MAX_CHARS + 50 ASCII chars.
        let long_title: String = "a".repeat(AGENT_OSC_MAX_CHARS + 50);
        let seq = format!("\x1b]0;{long_title}\x07");
        t.observe(seq.as_bytes());
        assert_eq!(t.latest_title().len(), AGENT_OSC_MAX_CHARS);
    }

    #[test]
    fn agent_osc_control_chars_stripped() {
        let mut t = AgentOscStateTracker::default();
        t.observe(b"\x1b]0;before\x01after\x07");
        assert_eq!(t.latest_title(), "beforeafter");
    }

    #[test]
    fn agent_osc_unrelated_osc_does_not_overwrite_title() {
        let mut t = AgentOscStateTracker::default();
        t.observe(b"\x1b]0;my title\x07");
        // OSC 4 (palette color), OSC 52 (clipboard) — should not touch title/progress.
        t.observe(b"\x1b]4;1;rgb:aa/bb/cc\x07");
        t.observe(b"\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(t.latest_title(), "my title");
        assert_eq!(t.latest_progress(), "");
    }

    #[test]
    fn agent_osc_interleaved_sequences() {
        let mut t = AgentOscStateTracker::default();
        // OSC 0 title, then OSC 9 progress, then OSC 2 title update.
        t.observe(b"\x1b]0;first\x07\x1b]9;4;3;\x07\x1b]2;second\x07");
        assert_eq!(t.latest_title(), "second");
        assert_eq!(t.latest_progress(), "4;3;");
    }

    #[test]
    fn agent_osc_default_state_is_empty() {
        let t = AgentOscStateTracker::default();
        assert_eq!(t.latest_title(), "");
        assert_eq!(t.latest_progress(), "");
    }
}
