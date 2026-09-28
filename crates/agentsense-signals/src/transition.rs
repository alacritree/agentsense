// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr for agentsense; see the package NOTICE.

//! Screen-state publication and idle confirmation without a timer or runtime.
//!
//! Adapted from Herdr's `src/pane/agent_detection.rs` (see NOTICE). Callers own
//! the clock, prior state and output sequence. History views preserve state;
//! an observed process exit takes precedence and clears stale evidence.

use std::sync::atomic::{AtomicU64, Ordering};

use agentsense_core::{Agent, AgentDetection, AgentState};

pub const AGENT_PENDING_IDLE_RECHECK: std::time::Duration = std::time::Duration::from_millis(100);
const AGENT_PENDING_IDLE_CONFIRMATIONS: u8 = 3;
pub const AGENT_PENDING_IDLE_CAP: std::time::Duration = std::time::Duration::from_millis(700);
pub const STABLE_VISIBLE_SIGNAL_REFRESH: std::time::Duration =
    std::time::Duration::from_millis(800);
pub const AGENT_STARTUP_GRACE_WINDOW: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectionPublishState {
    pub state: AgentState,
    pub visible_idle: bool,
    pub visible_blocker: bool,
    pub visible_working: bool,
}

#[derive(Debug, Default)]
pub struct PendingIdleConfirmation {
    started_at: Option<std::time::Instant>,
    confirmations: u8,
}

impl PendingIdleConfirmation {
    pub fn active(&self) -> bool {
        self.started_at.is_some()
    }

    pub fn clear(&mut self) {
        self.started_at = None;
        self.confirmations = 0;
    }

    pub fn should_hold_working_to_idle(
        &mut self,
        previous: DetectionPublishState,
        next: DetectionPublishState,
        agent_changed: bool,
        process_exited: bool,
        now: std::time::Instant,
    ) -> bool {
        let is_working_to_plain_idle = previous.state == AgentState::Working
            && next.state == AgentState::Idle
            && !next.visible_idle
            && !next.visible_blocker
            && !agent_changed
            && !process_exited;

        if !is_working_to_plain_idle {
            self.clear();
            return false;
        }

        let Some(started_at) = self.started_at else {
            self.started_at = Some(now);
            self.confirmations = 0;
            return true;
        };

        if now.duration_since(started_at) >= AGENT_PENDING_IDLE_CAP {
            self.clear();
            return false;
        }

        self.confirmations = self.confirmations.saturating_add(1);
        if self.confirmations >= AGENT_PENDING_IDLE_CONFIRMATIONS {
            self.clear();
            return false;
        }

        true
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IdleScreenScanSkipInput {
    pub state: AgentState,
    pub agent: Option<Agent>,
    pub pending_idle_active: bool,
    pub agent_changed: bool,
    pub process_exited: bool,
    pub current_detection_content_seq: Option<u64>,
    pub last_screen_scan_detection_content_seq: Option<u64>,
}

pub fn should_skip_idle_screen_scan(input: IdleScreenScanSkipInput) -> bool {
    let stable_state = input.state == AgentState::Idle
        || (input.state == AgentState::Unknown && input.agent == Some(Agent::Codex));
    if !stable_state
        || input.agent.is_none()
        || input.pending_idle_active
        || input.agent_changed
        || input.process_exited
    {
        return false;
    }

    input.current_detection_content_seq.is_some()
        && input.last_screen_scan_detection_content_seq == input.current_detection_content_seq
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionScreenReadDecision {
    Read,
    Skip,
}

#[derive(Debug, Clone, Copy)]
pub struct DetectionScreenReadInput {
    pub state: AgentState,
    pub agent: Option<Agent>,
    pub pending_idle_active: bool,
    pub agent_changed: bool,
    pub process_exited: bool,
    pub current_detection_content_seq: Option<u64>,
    pub last_screen_scan_detection_content_seq: Option<u64>,
}

pub fn decide_detection_screen_read(
    input: DetectionScreenReadInput,
) -> DetectionScreenReadDecision {
    if should_skip_idle_screen_scan(IdleScreenScanSkipInput {
        state: input.state,
        agent: input.agent,
        pending_idle_active: input.pending_idle_active,
        agent_changed: input.agent_changed,
        process_exited: input.process_exited,
        current_detection_content_seq: input.current_detection_content_seq,
        last_screen_scan_detection_content_seq: input.last_screen_scan_detection_content_seq,
    }) {
        DetectionScreenReadDecision::Skip
    } else {
        DetectionScreenReadDecision::Read
    }
}

pub fn should_publish_detection_update(
    previous: DetectionPublishState,
    next: DetectionPublishState,
    agent_changed: bool,
    process_exited: bool,
    stable_visible_signal_refresh_due: bool,
) -> bool {
    next.state != previous.state
        || next.visible_idle != previous.visible_idle
        || next.visible_blocker != previous.visible_blocker
        || next.visible_working != previous.visible_working
        || agent_changed
        || process_exited
        || (stable_visible_signal_refresh_due && next.visible_blocker && previous.visible_blocker)
}

pub fn stable_visible_signal_refresh_due(
    previous: DetectionPublishState,
    next: DetectionPublishState,
    last_refresh: Option<std::time::Instant>,
    now: std::time::Instant,
) -> bool {
    let stable_visible_signal = next.visible_blocker && previous.visible_blocker;

    stable_visible_signal
        && last_refresh.is_none_or(|last_refresh| {
            now.duration_since(last_refresh) >= STABLE_VISIBLE_SIGNAL_REFRESH
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionTransitionDecision {
    NoPublish,
    PublishNext,
}

#[derive(Debug, Clone, Copy)]
pub struct DetectionTransitionInput {
    pub previous_publish: DetectionPublishState,
    pub next_publish: DetectionPublishState,
    pub agent_changed: bool,
    pub process_exited: bool,
    pub stable_refresh_due: bool,
    pub now: std::time::Instant,
}

pub fn decide_detection_transition(
    input: DetectionTransitionInput,
    pending_idle: &mut PendingIdleConfirmation,
) -> DetectionTransitionDecision {
    if pending_idle.should_hold_working_to_idle(
        input.previous_publish,
        input.next_publish,
        input.agent_changed,
        input.process_exited,
        input.now,
    ) {
        return DetectionTransitionDecision::NoPublish;
    }

    if should_publish_detection_update(
        input.previous_publish,
        input.next_publish,
        input.agent_changed,
        input.process_exited,
        input.stable_refresh_due,
    ) {
        return DetectionTransitionDecision::PublishNext;
    }

    DetectionTransitionDecision::NoPublish
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionPublishDecision {
    NoPublish,
    Publish {
        state: AgentState,
        visible_idle: bool,
        visible_blocker: bool,
        visible_working: bool,
        process_exited: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenDetectionPublishInput {
    pub current_state: AgentState,
    pub last_visible_idle: bool,
    pub last_visible_blocker: bool,
    pub last_visible_working: bool,
    pub last_visible_signal_refresh: Option<std::time::Instant>,
    pub screen_detection: AgentDetection,
    pub process_exited: bool,
    pub agent_changed: bool,
    pub now: std::time::Instant,
}

pub fn decide_screen_detection_publish(
    input: ScreenDetectionPublishInput,
    pending_idle: &mut PendingIdleConfirmation,
) -> DetectionPublishDecision {
    let Some(detection) =
        detection_update_for_publish(input.screen_detection, input.process_exited)
    else {
        return DetectionPublishDecision::NoPublish;
    };
    let new_state = detection.state;
    let visible_idle = detection.visible_idle && new_state == AgentState::Idle;
    let visible_blocker = detection.visible_blocker && new_state == AgentState::Blocked;
    let visible_working = detection.visible_working && new_state == AgentState::Working;

    let previous_publish = DetectionPublishState {
        state: input.current_state,
        visible_idle: input.last_visible_idle,
        visible_blocker: input.last_visible_blocker,
        visible_working: input.last_visible_working,
    };
    let next_publish = DetectionPublishState {
        state: new_state,
        visible_idle,
        visible_blocker,
        visible_working,
    };
    let stable_refresh_due = stable_visible_signal_refresh_due(
        previous_publish,
        next_publish,
        input.last_visible_signal_refresh,
        input.now,
    );

    match decide_detection_transition(
        DetectionTransitionInput {
            previous_publish,
            next_publish,
            agent_changed: input.agent_changed,
            process_exited: input.process_exited,
            stable_refresh_due,
            now: input.now,
        },
        pending_idle,
    ) {
        DetectionTransitionDecision::NoPublish => DetectionPublishDecision::NoPublish,
        DetectionTransitionDecision::PublishNext => DetectionPublishDecision::Publish {
            state: new_state,
            visible_idle,
            visible_blocker,
            visible_working,
            process_exited: input.process_exited,
        },
    }
}

/// Apply process-exit and history-view rules to an already parsed screen result.
/// The host supplies the result from its detector and owns process observation.
pub fn detection_update_for_publish(
    detection: AgentDetection,
    process_exited: bool,
) -> Option<AgentDetection> {
    if process_exited {
        return Some(AgentDetection {
            state: AgentState::Idle,
            skip_state_update: false,
            visible_idle: true,
            visible_blocker: false,
            visible_working: false,
        });
    }
    (!detection.skip_state_update).then_some(detection)
}

pub fn codex_prompt_ready(content: &str) -> bool {
    // The composer can remain visible during a turn; this is startup evidence only.
    let recent: String = content
        .lines()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .flat_map(str::chars)
        .filter(|ch| !ch.is_whitespace())
        .collect();
    recent.contains("›AskCodextodoanything")
        && !recent.contains("model:loading")
        && !recent.contains("Resumingsession")
}

pub fn observe_detection_content_change(bytes: &[u8], detection_content_seq: &AtomicU64) {
    if !bytes.is_empty() {
        detection_content_seq.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn mark_detection_content_changed(detection_content_seq: &AtomicU64) {
    detection_content_seq.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_view_preserves_state_at_public_boundary() {
        let mut detection = screen_detection(AgentState::Unknown);
        detection.skip_state_update = true;
        let mut pending = PendingIdleConfirmation::default();
        assert_eq!(
            decide_screen_detection_publish(
                screen_publish_input(AgentState::Working, detection, std::time::Instant::now()),
                &mut pending,
            ),
            DetectionPublishDecision::NoPublish,
        );
    }

    #[test]
    fn process_exit_overrides_a_skipped_history_view() {
        let mut detection = screen_detection(AgentState::Unknown);
        detection.skip_state_update = true;
        let mut input =
            screen_publish_input(AgentState::Working, detection, std::time::Instant::now());
        input.process_exited = true;
        assert_eq!(
            decide_screen_detection_publish(input, &mut PendingIdleConfirmation::default()),
            DetectionPublishDecision::Publish {
                state: AgentState::Idle,
                visible_idle: true,
                visible_blocker: false,
                visible_working: false,
                process_exited: true,
            },
        );
    }

    #[test]
    fn plain_idle_confirmation_has_a_time_cap() {
        let now = std::time::Instant::now();
        let mut pending = PendingIdleConfirmation::default();
        let previous = publish_state(AgentState::Working);
        let next = publish_state(AgentState::Idle);
        assert!(pending.should_hold_working_to_idle(previous, next, false, false, now));
        assert!(!pending.should_hold_working_to_idle(
            previous,
            next,
            false,
            false,
            now + AGENT_PENDING_IDLE_CAP,
        ));
        assert!(!pending.active());
    }

    fn publish_state(state: AgentState) -> DetectionPublishState {
        DetectionPublishState {
            state,
            visible_idle: false,
            visible_blocker: false,
            visible_working: false,
        }
    }

    fn screen_detection(state: AgentState) -> AgentDetection {
        AgentDetection {
            state,
            skip_state_update: false,
            visible_idle: state == AgentState::Idle,
            visible_blocker: false,
            visible_working: state == AgentState::Working,
        }
    }

    fn screen_publish_input(
        current_state: AgentState,
        screen_detection: AgentDetection,
        now: std::time::Instant,
    ) -> ScreenDetectionPublishInput {
        ScreenDetectionPublishInput {
            current_state,
            last_visible_idle: false,
            last_visible_blocker: false,
            last_visible_working: false,
            last_visible_signal_refresh: None,
            screen_detection,
            process_exited: false,
            agent_changed: false,
            now,
        }
    }

    fn screen_read_input(state: AgentState, current_seq: u64) -> DetectionScreenReadInput {
        DetectionScreenReadInput {
            state,
            agent: Some(Agent::Codex),
            pending_idle_active: false,
            agent_changed: false,
            process_exited: false,
            current_detection_content_seq: Some(current_seq),
            last_screen_scan_detection_content_seq: Some(10),
        }
    }

    #[test]
    fn codex_startup_prompt_survives_terminal_wraps() {
        let wrapped = "header\n› Ask Codex to do\nanything\nfooter";
        assert!(codex_prompt_ready(wrapped));
        assert!(!codex_prompt_ready(&format!("model: load\ning\n{wrapped}")));
    }

    #[test]
    fn screen_read_skips_unchanged_idle_bottom_buffer() {
        assert_eq!(
            decide_detection_screen_read(screen_read_input(AgentState::Idle, 10)),
            DetectionScreenReadDecision::Skip
        );
    }

    #[test]
    fn screen_read_skips_unchanged_ambiguous_codex_but_not_new_content_or_replacement() {
        let mut input = screen_read_input(AgentState::Unknown, 10);
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Skip
        );
        input.current_detection_content_seq = Some(11);
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Read
        );
        input.current_detection_content_seq = Some(10);
        input.agent_changed = true;
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Read
        );
        input.agent_changed = false;
        input.agent = Some(Agent::Pi);
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Read
        );
    }

    #[test]
    fn screen_read_reads_when_idle_bottom_buffer_changes() {
        assert_eq!(
            decide_detection_screen_read(screen_read_input(AgentState::Idle, 11)),
            DetectionScreenReadDecision::Read
        );
    }

    #[test]
    fn screen_read_reads_for_transitions_and_missing_agent() {
        let mut input = screen_read_input(AgentState::Idle, 10);
        input.pending_idle_active = true;
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Read
        );

        let mut input = screen_read_input(AgentState::Idle, 10);
        input.agent_changed = true;
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Read
        );

        let mut input = screen_read_input(AgentState::Idle, 10);
        input.process_exited = true;
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Read
        );

        let mut input = screen_read_input(AgentState::Idle, 10);
        input.agent = None;
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Read
        );
    }

    #[test]
    fn pending_idle_holds_working_to_plain_idle_until_confirmed() {
        let now = std::time::Instant::now();
        let previous = publish_state(AgentState::Working);
        let next = publish_state(AgentState::Idle);
        let mut pending = PendingIdleConfirmation::default();

        assert!(pending.should_hold_working_to_idle(previous, next, false, false, now));
        assert!(pending.should_hold_working_to_idle(
            previous,
            next,
            false,
            false,
            now + AGENT_PENDING_IDLE_RECHECK
        ));
        assert!(pending.should_hold_working_to_idle(
            previous,
            next,
            false,
            false,
            now + AGENT_PENDING_IDLE_RECHECK * 2
        ));
        assert!(!pending.should_hold_working_to_idle(
            previous,
            next,
            false,
            false,
            now + AGENT_PENDING_IDLE_RECHECK * 3
        ));
    }

    #[test]
    fn visible_idle_bypasses_plain_idle_hold() {
        let now = std::time::Instant::now();
        let previous = publish_state(AgentState::Working);
        let mut next = publish_state(AgentState::Idle);
        next.visible_idle = true;
        let mut pending = PendingIdleConfirmation::default();

        assert!(!pending.should_hold_working_to_idle(previous, next, false, false, now));
    }

    #[test]
    fn transition_decision_publishes_next_for_visible_blocker() {
        let now = std::time::Instant::now();
        let mut pending_idle = PendingIdleConfirmation::default();
        let mut blocked = publish_state(AgentState::Blocked);
        blocked.visible_blocker = true;

        assert_eq!(
            decide_detection_transition(
                DetectionTransitionInput {
                    previous_publish: publish_state(AgentState::Idle),
                    next_publish: blocked,
                    agent_changed: false,
                    process_exited: false,
                    stable_refresh_due: false,
                    now,
                },
                &mut pending_idle,
            ),
            DetectionTransitionDecision::PublishNext
        );
    }

    #[test]
    fn screen_publish_keeps_visible_working_without_pty_activity() {
        let now = std::time::Instant::now();
        let mut pending_idle = PendingIdleConfirmation::default();

        assert_eq!(
            decide_screen_detection_publish(
                screen_publish_input(AgentState::Idle, screen_detection(AgentState::Working), now,),
                &mut pending_idle,
            ),
            DetectionPublishDecision::Publish {
                state: AgentState::Working,
                visible_idle: false,
                visible_blocker: false,
                visible_working: true,
                process_exited: false,
            }
        );
    }

    #[test]
    fn screen_publish_can_publish_idle_without_input_taint_delay() {
        let now = std::time::Instant::now();
        let mut pending_idle = PendingIdleConfirmation::default();

        assert_eq!(
            decide_screen_detection_publish(
                screen_publish_input(AgentState::Blocked, screen_detection(AgentState::Idle), now,),
                &mut pending_idle,
            ),
            DetectionPublishDecision::Publish {
                state: AgentState::Idle,
                visible_idle: true,
                visible_blocker: false,
                visible_working: false,
                process_exited: false,
            }
        );
    }

    #[test]
    fn detection_content_change_tracks_raw_nonempty_reads_for_scan_scheduling() {
        let seq = AtomicU64::new(0);

        observe_detection_content_change(b"", &seq);
        assert_eq!(seq.load(Ordering::Relaxed), 0);

        observe_detection_content_change(b"\x1b[?2026h", &seq);
        assert_eq!(seq.load(Ordering::Relaxed), 1);

        observe_detection_content_change(b"body bytes", &seq);
        assert_eq!(seq.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn local_terminal_mutations_can_invalidate_idle_scan_skip() {
        let seq = AtomicU64::new(0);

        mark_detection_content_changed(&seq);

        assert_eq!(seq.load(Ordering::Relaxed), 1);
    }
}
