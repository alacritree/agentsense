// SPDX-License-Identifier: Apache-2.0

//! In-memory agent status and attention state for one terminal session.
//! The host supplies monotonically increasing time and accepted title changes.

use crate::Agent;
use std::time::{Duration, Instant};

/// State of a present agent. `Finished` means its current turn completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Working,
    Finished,
    WaitingForAction,
}

/// State reported by a hook or external pane owner such as Herdr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportedStatus {
    Idle,
    Working,
    Blocked,
    Done,
    Unknown,
}

/// A trusted reporter (hook or pane owner) establishes agent presence even if the agent name is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusReport {
    pub agent: Option<Agent>,
    pub status: ReportedStatus,
}

/// Host observations relevant to agent tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    /// Result of the latest foreground process probe. `None` clears a stale identity.
    Process(Option<Agent>),
    /// A terminal title change already accepted by the host (for example, not pinned).
    Title(&'a str),
    /// Bell requests attention but does not prove the agent is blocked.
    Bell,
    /// Latest trusted status report; `None` means the source detached.
    Report(Option<StatusReport>),
    /// Whether the user can currently see this terminal in a focused window.
    Viewed(bool),
    /// The child process exited; old process, report and title evidence is invalid.
    Exit,
    /// Poll after a requested delay to mature a pending completion or bell.
    Tick,
}

/// Current agent and attention reading. `status` is `None` when no agent is
/// present or when a trusted source reports an unknown state with no other clue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub agent_present: bool,
    pub agent: Option<Agent>,
    pub status: Option<Status>,
    pub needs_attention: bool,
}

/// Result of handling one event. The host should schedule `next_check` and
/// emit a notification when `notify` becomes true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Update {
    pub snapshot: Snapshot,
    pub notify: bool,
    pub next_check: Option<Duration>,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    since: Instant,
    finished: bool,
    rang: bool,
}

/// One terminal's agent tracking state.
#[derive(Debug, Default)]
pub struct Tracker {
    process_agent: Option<Agent>,
    report: Option<StatusReport>,
    title_spinner: bool,
    viewed: bool,
    finished: bool,
    attention: bool,
    pending: Option<Pending>,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Snapshot {
        let agent_present = self.process_agent.is_some() || self.report.is_some();
        let agent = self
            .report
            .and_then(|pane| pane.agent)
            .or(self.process_agent);
        let status = if !agent_present {
            None
        } else {
            match self.report.map(|pane| pane.status) {
                Some(ReportedStatus::Working) => Some(Status::Working),
                Some(ReportedStatus::Blocked) => Some(Status::WaitingForAction),
                Some(ReportedStatus::Done) => Some(Status::Finished),
                Some(ReportedStatus::Idle) => Some(if self.finished {
                    Status::Finished
                } else {
                    Status::Idle
                }),
                Some(ReportedStatus::Unknown) | None if self.title_spinner => Some(Status::Working),
                Some(ReportedStatus::Unknown) | None if self.finished => Some(Status::Finished),
                Some(ReportedStatus::Unknown) if self.process_agent.is_none() => None,
                _ => Some(Status::Idle),
            }
        };
        Snapshot {
            agent_present,
            agent,
            status,
            needs_attention: self.attention,
        }
    }

    /// Apply an event at the host's current monotonic time.
    pub fn observe(&mut self, event: Event<'_>, now: Instant, grace: Duration) -> Update {
        let before = self.finished || self.attention;
        match event {
            Event::Process(agent) => {
                if self.process_agent != agent {
                    self.finished = false;
                    self.process_agent = agent;
                }
            }
            Event::Title(title) => {
                let spinning = title
                    .chars()
                    .any(|c| (0x2800..=0x28ff).contains(&(c as u32)));
                if self.title_spinner && !spinning && !self.viewed {
                    self.merge_pending(now, true, false);
                }
                self.title_spinner = spinning;
            }
            Event::Bell if !self.viewed => self.merge_pending(now, false, true),
            Event::Report(report) => {
                if self.report != report {
                    self.finished = false;
                }
                self.report = report;
                if !self.viewed {
                    match report.map(|r| r.status) {
                        Some(ReportedStatus::Done) => self.finished = true,
                        Some(ReportedStatus::Blocked) => self.attention = true,
                        _ => {}
                    }
                }
            }
            Event::Viewed(viewed) => {
                self.viewed = viewed;
                if viewed {
                    self.pending = None;
                    self.finished = false;
                    self.attention = false;
                }
            }
            Event::Exit => {
                self.process_agent = None;
                self.report = None;
                self.title_spinner = false;
                self.pending = None;
                self.finished = false;
            }
            Event::Bell | Event::Tick => {}
        }
        if self.snapshot().status == Some(Status::Working) {
            self.pending = None;
            self.finished = false;
        }
        let mut next_check = None;
        if !self.viewed {
            if let Some(pending) = self.pending {
                let elapsed = now.saturating_duration_since(pending.since);
                if grace.is_zero() || elapsed >= grace {
                    self.pending = None;
                    self.finished |= pending.finished && self.snapshot().agent_present;
                    self.attention |=
                        pending.rang || (pending.finished && !self.snapshot().agent_present);
                } else {
                    next_check = Some(grace - elapsed);
                }
            }
        }
        Update {
            snapshot: self.snapshot(),
            notify: !before && (self.finished || self.attention),
            next_check,
        }
    }

    fn merge_pending(&mut self, now: Instant, finished: bool, rang: bool) {
        match &mut self.pending {
            Some(pending) => {
                pending.finished |= finished;
                pending.rang |= rang;
            }
            None => {
                self.pending = Some(Pending {
                    since: now,
                    finished,
                    rang,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_turn_finishes_after_grace_and_resets_on_new_work() {
        let now = Instant::now();
        let grace = Duration::from_millis(100);
        let mut tracker = Tracker::new();
        tracker.observe(Event::Process(Some(Agent::Claude)), now, grace);
        assert_eq!(tracker.snapshot().status, Some(Status::Idle));
        tracker.observe(Event::Title("⠋ Claude"), now, grace);
        assert_eq!(tracker.snapshot().status, Some(Status::Working));
        let update = tracker.observe(Event::Title("Claude"), now, grace);
        assert_eq!(update.next_check, Some(grace));
        assert_eq!(update.snapshot.status, Some(Status::Idle));
        let update = tracker.observe(Event::Tick, now + grace, grace);
        assert!(update.notify);
        assert_eq!(update.snapshot.status, Some(Status::Finished));
        tracker.observe(Event::Title("⠙ Claude"), now + grace, grace);
        assert_eq!(tracker.snapshot().status, Some(Status::Working));
    }

    #[test]
    fn resumed_work_cancels_completion_and_bell_does_not_mean_blocked() {
        let now = Instant::now();
        let grace = Duration::from_millis(100);
        let mut tracker = Tracker::new();
        tracker.observe(Event::Process(Some(Agent::Codex)), now, grace);
        tracker.observe(Event::Title("⠋ Codex"), now, grace);
        tracker.observe(Event::Title("Codex"), now, grace);
        tracker.observe(
            Event::Title("⠙ Codex"),
            now + Duration::from_millis(50),
            grace,
        );
        assert_eq!(
            tracker
                .observe(Event::Tick, now + grace, grace)
                .snapshot
                .status,
            Some(Status::Working)
        );
        tracker.observe(Event::Title("Codex"), now + grace, grace);
        tracker.observe(Event::Bell, now + grace, grace);
        let update = tracker.observe(Event::Tick, now + grace + grace, grace);
        assert_eq!(update.snapshot.status, Some(Status::Finished));
        assert!(update.snapshot.needs_attention);
    }

    #[test]
    fn pane_status_has_authority_and_unknown_falls_back() {
        let now = Instant::now();
        let mut tracker = Tracker::new();
        let zero = Duration::ZERO;
        tracker.observe(
            Event::Report(Some(StatusReport {
                agent: None,
                status: ReportedStatus::Unknown,
            })),
            now,
            zero,
        );
        assert!(tracker.snapshot().agent_present);
        assert_eq!(tracker.snapshot().status, None);
        tracker.observe(
            Event::Report(Some(StatusReport {
                agent: None,
                status: ReportedStatus::Blocked,
            })),
            now,
            zero,
        );
        assert_eq!(tracker.snapshot().status, Some(Status::WaitingForAction));
        tracker.observe(
            Event::Report(Some(StatusReport {
                agent: None,
                status: ReportedStatus::Done,
            })),
            now,
            zero,
        );
        assert_eq!(tracker.snapshot().status, Some(Status::Finished));
        tracker.observe(Event::Exit, now, zero);
        assert!(!tracker.snapshot().agent_present);
    }

    #[test]
    fn trusted_reports_drive_blocked_and_done_notifications() {
        let now = Instant::now();
        let mut tracker = Tracker::new();
        let zero = Duration::ZERO;
        let blocked = StatusReport {
            agent: Some(Agent::Kimi),
            status: ReportedStatus::Blocked,
        };
        let update = tracker.observe(Event::Report(Some(blocked)), now, zero);
        assert_eq!(update.snapshot.status, Some(Status::WaitingForAction));
        assert!(update.notify);
        tracker.observe(Event::Viewed(true), now, zero);
        let done = StatusReport {
            agent: Some(Agent::Kimi),
            status: ReportedStatus::Done,
        };
        let update = tracker.observe(Event::Report(Some(done)), now, zero);
        assert_eq!(update.snapshot.status, Some(Status::Finished));
        assert!(!update.notify); // the user is viewing this terminal
    }

    #[test]
    fn seeing_terminal_clears_finished_attention() {
        let now = Instant::now();
        let mut tracker = Tracker::new();
        let zero = Duration::ZERO;
        tracker.observe(Event::Process(Some(Agent::Pi)), now, zero);
        tracker.observe(Event::Bell, now, zero);
        assert!(tracker.snapshot().needs_attention);
        tracker.observe(Event::Viewed(true), now, zero);
        assert!(!tracker.snapshot().needs_attention);
    }
}
