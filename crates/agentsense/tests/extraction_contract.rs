//! Public API contracts across process, OSC, manifest and publication boundaries.

use agentsense::signals::{transition::*, AgentOscStateTracker};
use agentsense::{
    identify_agent_in_job, Agent, AgentState, DetectionInput, Detector, ForegroundJob,
    ForegroundProcess,
};

const RULES: &str = r#"
id = "codex"
[[rules]]
id = "title-working"
state = "working"
region = "osc_title"
visible_working = true
contains = ["processing"]
priority = 1

[[rules]]
id = "live-approval"
state = "blocked"
region = "bottom_non_empty_lines(2)"
visible_blocker = true
contains = ["approval required"]
priority = 2

[[rules]]
id = "history"
state = "unknown"
region = "bottom_lines(1)"
skip_state_update = true
contains = ["history view"]
priority = 3
"#;

#[test]
fn process_and_split_osc_signals_flow_through_detector_and_publication() {
    let job = ForegroundJob {
        process_group_id: 7,
        processes: vec![ForegroundProcess {
            pid: 7,
            name: "node".into(),
            argv0: None,
            argv: Some(vec!["node".into(), "/tools/codex.js".into()]),
            cmdline: None,
        }],
    };
    let (agent, _) = identify_agent_in_job(&job).expect("wrapped agent");
    assert_eq!(agent, Agent::Codex);

    let mut detector = Detector::empty();
    detector.set_manifest(agent, RULES).unwrap();
    let mut osc = AgentOscStateTracker::default();
    osc.observe(b"\x1b]2;Proce");
    osc.observe(b"ssing\x1b");
    osc.observe(b"\\");
    let input = DetectionInput {
        screen: "live output",
        osc_title: osc.latest_title(),
        osc_progress: osc.latest_progress(),
    };
    let detection = detector.detect(Some(agent), input);
    assert_eq!(detection.state, AgentState::Working);
    assert_eq!(detection, detector.explain(Some(agent), input).detection());

    let now = std::time::Instant::now();
    let mut publish = ScreenDetectionPublishInput {
        current_state: AgentState::Idle,
        last_visible_idle: true,
        last_visible_blocker: false,
        last_visible_working: false,
        last_visible_signal_refresh: None,
        screen_detection: detection,
        process_exited: false,
        agent_changed: false,
        now,
    };
    let mut pending = PendingIdleConfirmation::default();
    assert!(matches!(
        decide_screen_detection_publish(publish, &mut pending),
        DetectionPublishDecision::Publish {
            state: AgentState::Working,
            ..
        }
    ));

    let blocked = DetectionInput {
        screen: "Approval required",
        ..input
    };
    publish.current_state = AgentState::Working;
    publish.screen_detection = detector.detect(Some(agent), blocked);
    assert!(matches!(
        decide_screen_detection_publish(publish, &mut pending),
        DetectionPublishDecision::Publish {
            state: AgentState::Blocked,
            ..
        }
    ));

    let history = DetectionInput {
        screen: "Approval required\nHistory view",
        ..input
    };
    publish.current_state = AgentState::Blocked;
    publish.screen_detection = detector.detect(Some(agent), history);
    assert_eq!(
        decide_screen_detection_publish(publish, &mut pending),
        DetectionPublishDecision::NoPublish
    );
    publish.process_exited = true;
    assert!(matches!(
        decide_screen_detection_publish(publish, &mut pending),
        DetectionPublishDecision::Publish {
            state: AgentState::Idle,
            process_exited: true,
            ..
        }
    ));
}

#[test]
fn detector_can_be_shared_across_terminal_workers() {
    fn send_and_sync<T: Send + Sync>() {}
    send_and_sync::<Detector>();
    let mut detector = Detector::empty();
    detector.set_manifest(Agent::Codex, RULES).unwrap();
    std::thread::scope(|scope| {
        let working = scope.spawn(|| {
            detector.detect(
                Some(Agent::Codex),
                DetectionInput {
                    screen: "output",
                    osc_title: "Processing",
                    osc_progress: "",
                },
            )
        });
        let blocked = scope.spawn(|| {
            detector.detect(
                Some(Agent::Codex),
                DetectionInput::screen("Approval required"),
            )
        });
        assert_eq!(working.join().unwrap().state, AgentState::Working);
        assert_eq!(blocked.join().unwrap().state, AgentState::Blocked);
    });
}
