// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr for agentsense; see the package NOTICE.

use crate::manifest::*;
use crate::regions::*;
use crate::*;

#[test]
fn screen_regions_extract_structure_without_classifying_agent_state() {
    for (screen, spec, expected) in [
        ("old\n\nnew\n", "bottom_lines(2)", "\nnew\n"),
        (
            "before\n› input\nafter\n",
            "after_last_prompt_marker",
            "after\n",
        ),
        (
            "before\n› input\nafter\n",
            "before_current_prompt_marker",
            "before\n",
        ),
        (
            "before\n› input\nafter\n",
            "whole_recent_without_current_prompt_marker",
            "",
        ),
        (
            "no marker\n",
            "whole_recent_without_current_prompt_marker",
            "no marker\n",
        ),
        (
            "• old\n■ latest\n› input\n",
            "current_prompt_block_marker",
            "■ latest",
        ),
        (
            "• old\n■ latest\n› input\n",
            "after_current_prompt_block_marker",
            "■ latest\n› input\n",
        ),
        ("› old\n• new\n", "current_prompt_block_marker", ""),
        (
            "above\n\n───\nbody\n───\nfooter\n",
            "above_prompt_box",
            "above\n\n",
        ),
        (
            "above\n\n───\nbody\n───\nfooter\n",
            "last_non_empty_above_prompt_box",
            "above",
        ),
        (
            "above\n───\nbody\n───\nfooter\n",
            "prompt_box_body",
            "body\n",
        ),
        (
            "above\n───\nbody\n───\nfooter\n",
            "after_last_horizontal_rule",
            "footer\n",
        ),
    ] {
        assert_eq!(
            region(
                DetectionInput {
                    screen,
                    osc_title: "",
                    osc_progress: ""
                },
                spec
            ),
            expected,
            "region={spec}"
        );
    }
}

#[test]
fn all_bundled_manifests_parse_and_validate() {
    for agent in Agent::SCREEN_MANIFEST_AGENTS {
        assert!(
            bundled_manifest(agent).is_some(),
            "missing bundled manifest for {}",
            agent_label(agent)
        );
    }
}

#[test]
fn manifest_validation_rejects_unknown_fields_empty_rules_invalid_regions_and_regexes() {
    assert!(Manifest::parse(
        r#"
id = "codex"

[[rules]]
id = "typo"
state = "working"
contain = ["Working"]
"#
    )
    .is_err());
    assert!(Manifest::parse(
        r#"
id = "codex"

[[rules]]
id = "empty"
state = "working"
"#
    )
    .is_err());
    assert!(Manifest::parse(
        r#"
id = "codex"

[[rules]]
id = "bad_region"
state = "working"
region = "after_last_promt_marker"
contains = ["Working"]
"#
    )
    .is_err());
    assert!(Manifest::parse(
        r#"
id = "codex"

[[rules]]
id = "bad_regex"
state = "working"
regex = ["["]
"#
    )
    .is_err());
    assert!(Manifest::parse(
        r#"
id = "codex"

[[rules]]
id = "bad_nested_regex"
state = "working"
any = [{ line_regex = ["["] }]
"#
    )
    .is_err());
}

#[test]
fn manifest_validation_keeps_skip_rules_neutral() {
    assert!(Manifest::parse(
        r#"
id = "codex"

[[rules]]
id = "bad_skip_state"
state = "idle"
skip_state_update = true
contains = ["menu"]
"#
    )
    .is_err());
    assert!(Manifest::parse(
        r#"
id = "codex"

[[rules]]
id = "bad_skip_visible"
state = "unknown"
skip_state_update = true
visible_blocker = true
contains = ["menu"]
"#
    )
    .is_err());
}

#[test]
fn manifest_validation_rejects_excessive_rule_count() {
    let mut manifest = String::from(
        r#"
id = "codex"
"#,
    );
    for index in 0..129 {
        manifest.push_str(&format!(
            r#"
[[rules]]
id = "rule_{index}"
state = "idle"
contains = ["ready"]
"#
        ));
    }
    assert!(Manifest::parse(&manifest).is_err());
}

#[test]
fn manifest_validation_rejects_excessive_gate_depth() {
    let manifest = r#"
id = "codex"

[[rules]]
id = "deep"
state = "idle"
contains = ["ready"]
all = [
  { contains = ["1"], all = [
    { contains = ["2"], all = [
      { contains = ["3"], all = [
        { contains = ["4"], all = [
          { contains = ["5"], all = [
            { contains = ["6"], all = [
              { contains = ["7"], all = [
                { contains = ["8"], all = [
                  { contains = ["9"] },
                ] },
              ] },
            ] },
          ] },
        ] },
      ] },
    ] },
  ] },
]
"#;
    assert!(Manifest::parse(manifest).is_err());
}

#[test]
fn manifest_validation_rejects_excessive_matchers() {
    let matchers = (0..33)
        .map(|index| format!(r#""m{index}""#))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = format!(
        r#"
id = "codex"

[[rules]]
id = "many"
state = "idle"
contains = [{matchers}]
"#
    );
    assert!(Manifest::parse(&manifest).is_err());
}

#[test]
fn bottom_non_empty_lines_uses_bottom_occurrence_for_repeated_text() {
    let content = "marker\nold\n\nmiddle\nmarker\nnew\n";
    assert_eq!(
        region(
            DetectionInput {
                screen: content,
                osc_title: "",
                osc_progress: ""
            },
            "bottom_non_empty_lines(2)"
        ),
        "marker\nnew\n"
    );
}

#[test]
fn top_non_empty_lines_uses_top_occurrence_for_repeated_text() {
    let content = "\nmarker\nold\n\nmiddle\nmarker\nnew\n";
    assert_eq!(
        region(
            DetectionInput {
                screen: content,
                osc_title: "",
                osc_progress: ""
            },
            "top_non_empty_lines(2)"
        ),
        "\nmarker\nold\n"
    );
}

#[test]
fn top_non_empty_lines_requires_a_canonical_positive_bounded_count() {
    let name = "top_non_empty_lines";
    assert!(validate_region_name(&format!("{name}(1)")).is_ok());
    assert!(validate_region_name(&format!("{name}({})", u16::MAX)).is_ok());
    for count in ["0", "01", "+1", "65536", "999999999999999999999999"] {
        assert!(
            validate_region_name(&format!("{name}({count})")).is_err(),
            "{name} accepted invalid count {count}"
        );
    }
}

#[test]
fn top_non_empty_lines_requires_engine_three_when_declared() {
    let manifest = r#"
id = "codex"
version = "1"
min_engine_version = 2

[[rules]]
id = "background"
state = "working"
region = " top_non_empty_lines(1) "
contains = ["active"]
"#;
    assert!(Manifest::parse(manifest).is_err());
}

fn rules_manifest(rules: &str) -> String {
    format!("id = \"codex\"\n{rules}")
}

fn explain(detector: &Detector, agent: Agent, screen: &str) -> DetectionExplain {
    detector.explain(Some(agent), DetectionInput::screen(screen))
}

#[test]
fn rule_semantics_apply_gates_priority_and_line_regex() {
    let mut detector = Detector::empty();
    detector
        .set_manifest(
            Agent::Codex,
            &rules_manifest(
                r#"
[[rules]]
id = "low_contains"
state = "idle"
priority = 1
contains = ["match"]

[[rules]]
id = "high_nested_gates"
state = "working"
priority = 10
contains = ["match"]
all = [
  { any = [{ regex = ["w[io]n"] }, { contains = ["fallback"] }] },
]
not = [
  { contains = ["blocked"] },
]

[[rules]]
id = "line_regex"
state = "blocked"
priority = 20
line_regex = ["^exact line$"]
"#,
            ),
        )
        .unwrap();

    let high = explain(&detector, Agent::Codex, "match win");
    assert_eq!(high.state, AgentState::Working);
    assert_eq!(
        high.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("high_nested_gates")
    );

    let not_gate = explain(&detector, Agent::Codex, "match win blocked");
    assert_eq!(not_gate.state, AgentState::Idle);
    assert_eq!(
        not_gate.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("low_contains")
    );

    let line = explain(&detector, Agent::Codex, "before\nexact line\nafter");
    assert_eq!(line.state, AgentState::Blocked);
    assert_eq!(
        line.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("line_regex")
    );
}

#[test]
fn osc_regions_use_separate_inputs_and_share_rule_priority() {
    let mut detector = Detector::empty();
    detector
        .set_manifest(
            Agent::Codex,
            &rules_manifest(
                r#"
[[rules]]
id = "screen"
state = "idle"
priority = 10
region = "whole_recent"
visible_idle = true
contains = ["screen-marker"]

[[rules]]
id = "title"
state = "working"
priority = 20
region = "osc_title"
visible_working = true
regex = ['^title-marker$']

[[rules]]
id = "progress"
state = "blocked"
priority = 30
region = "osc_progress"
visible_blocker = true
regex = ['^progress-marker$']
"#,
            ),
        )
        .unwrap();
    for (screen, title, progress, state, rule) in [
        ("screen-marker", "", "", AgentState::Idle, "screen"),
        (
            "screen-marker",
            "title-marker",
            "",
            AgentState::Working,
            "title",
        ),
        (
            "screen-marker",
            "title-marker",
            "progress-marker",
            AgentState::Blocked,
            "progress",
        ),
        (
            "screen-marker title-marker progress-marker",
            "",
            "",
            AgentState::Idle,
            "screen",
        ),
    ] {
        let input = DetectionInput {
            screen,
            osc_title: title,
            osc_progress: progress,
        };
        let result = detector.explain(Some(Agent::Codex), input);
        assert_eq!(result.state, state);
        assert_eq!(
            result
                .matched_rule
                .as_ref()
                .map(|matched| matched.id.as_str()),
            Some(rule)
        );
        let detection = detector.detect(Some(Agent::Codex), input);
        assert_eq!(detection.state, state);
        assert_eq!(detection.visible_idle, state == AgentState::Idle);
        assert_eq!(detection.visible_working, state == AgentState::Working);
        assert_eq!(detection.visible_blocker, state == AgentState::Blocked);
    }
    let swapped = detector.explain(
        Some(Agent::Codex),
        DetectionInput {
            screen: "",
            osc_title: "progress-marker",
            osc_progress: "title-marker",
        },
    );
    assert!(swapped.matched_rule.is_none());
}

#[test]
fn skip_rule_suppresses_state_update_without_visible_state_evidence() {
    let mut detector = Detector::empty();
    detector
        .set_manifest(
            Agent::Codex,
            &rules_manifest(
                r#"
[[rules]]
id = "activity"
state = "working"
priority = 10
visible_working = true
contains = ["activity-marker"]

[[rules]]
id = "overlay"
state = "unknown"
priority = 20
skip_state_update = true
contains = ["overlay-marker"]
"#,
            ),
        )
        .unwrap();
    let screen = "activity-marker overlay-marker";
    let result = explain(&detector, Agent::Codex, screen);
    assert_eq!(result.state, AgentState::Unknown);
    assert!(result.skip_state_update);
    assert_eq!(
        result.skipped_update_reason.as_deref(),
        Some("matched_rule:overlay")
    );
    assert!(!result.visible_idle);
    assert!(!result.visible_working);
    assert!(!result.visible_blocker);
    assert!(
        detector
            .detect(Some(Agent::Codex), DetectionInput::screen(screen))
            .skip_state_update
    );
}

fn local_manifest(state: &str, marker: &str) -> String {
    rules_manifest(&format!(
        "[[rules]]\nid = 'test'\nstate = '{state}'\ncontains = ['{marker}']\n"
    ))
}

#[test]
fn known_agent_fallbacks_do_not_assert_visible_evidence() {
    let detector = Detector::empty();
    for agent in std::iter::once(None).chain(Agent::ALL.into_iter().map(Some)) {
        let input = DetectionInput::screen("no matching rules");
        let result = detector.explain(agent, input);
        assert_eq!(result.detection(), detector.detect(agent, input));
        let ambiguous = matches!(agent, None | Some(Agent::Codex));
        assert_eq!(
            result.state,
            if ambiguous {
                AgentState::Unknown
            } else {
                AgentState::Idle
            }
        );
        assert!(!result.visible_idle);
        assert!(!result.visible_blocker);
        assert!(!result.visible_working);
        assert!(!result.skip_state_update);
        assert_eq!(
            result.fallback_reason.as_deref(),
            match agent {
                None => None,
                Some(Agent::Codex) => Some("codex_state_ambiguous"),
                Some(_) => Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK),
            }
        );
    }
    let unknown = detector.explain_for_label("future-agent", "".into());
    assert_eq!(unknown.agent.as_deref(), Some("future-agent"));
    assert_eq!(unknown.fallback_reason.as_deref(), Some("unknown_agent"));
}

#[test]
fn replacing_manifests_is_atomic_and_isolated_between_detectors() {
    let mut original = Detector::empty();
    original
        .set_manifest(Agent::Codex, &local_manifest("working", "ready"))
        .unwrap();
    let mut other = original.clone();
    other
        .set_manifest(Agent::Codex, &local_manifest("blocked", "ready"))
        .unwrap();
    for invalid in [
        "id = ".to_owned(),
        local_manifest("idle", "ready").replace("codex", "claude"),
    ] {
        assert!(original.set_manifest(Agent::Codex, &invalid).is_err());
        assert_eq!(
            original.detect(Some(Agent::Codex), "ready".into()).state,
            AgentState::Working
        );
    }
    assert_eq!(
        other.detect(Some(Agent::Codex), "ready".into()).state,
        AgentState::Blocked
    );
    assert_eq!(
        original.explain(Some(Agent::Codex), "ready".into()).source,
        Some(ManifestSource::Supplied)
    );
    original.reset_manifest(Agent::Codex);
    assert_eq!(
        original.explain(Some(Agent::Codex), "ready".into()).source,
        Some(ManifestSource::Bundled)
    );
    assert_eq!(
        other.detect(Some(Agent::Codex), "ready".into()).state,
        AgentState::Blocked
    );
}

#[test]
fn first_matching_rule_wins_equal_priorities_and_contains_ignores_case() {
    let mut detector = Detector::empty();
    detector
        .set_manifest(
            Agent::Codex,
            &rules_manifest(
                r#"
[[rules]]
id = "first"
state = "working"
priority = 10
contains = ["ÄCTIVE", "MARKER"]
[[rules]]
id = "second"
state = "blocked"
priority = 10
contains = ["marker"]
"#,
            ),
        )
        .unwrap();
    let input = DetectionInput::screen("äctive marker");
    let result = detector.explain(Some(Agent::Codex), input);
    assert_eq!(result.state, AgentState::Working);
    assert_eq!(result.matched_rule.as_ref().unwrap().id, "first");
    assert!(result.evaluated_rules.iter().all(|rule| rule.matched));
    assert_eq!(
        result.detection(),
        detector.detect(Some(Agent::Codex), input)
    );
}

#[test]
fn compiled_manifests_support_shared_readers() {
    let mut detector = Detector::empty();
    detector
        .set_manifest(Agent::Codex, &local_manifest("working", "active"))
        .unwrap();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let detector = &detector;
            scope.spawn(move || {
                for _ in 0..8 {
                    assert_eq!(
                        detector.detect(Some(Agent::Codex), "active".into()).state,
                        AgentState::Working
                    );
                }
            });
        }
    });
}

#[test]
fn fallback_retains_active_manifest_version() {
    let mut detector = Detector::empty();
    let text = format!(
        "version = '2026.01.02.3'\nmin_engine_version = 3\n{}",
        local_manifest("working", "active")
    );
    detector.set_manifest(Agent::Codex, &text).unwrap();
    let result = detector.explain(Some(Agent::Codex), "unmatched".into());
    assert_eq!(result.state, AgentState::Unknown);
    assert_eq!(result.source, Some(ManifestSource::Supplied));
    assert_eq!(
        result
            .manifest_version
            .as_ref()
            .map(ManifestVersion::as_str),
        Some("2026.01.02.3")
    );
}

#[test]
fn explanation_preview_is_bounded_by_unicode_characters() {
    let mut detector = Detector::empty();
    detector
        .set_manifest(Agent::Codex, &local_manifest("working", "界"))
        .unwrap();
    let screen = "界".repeat(241);
    let result = detector.explain(Some(Agent::Codex), screen.as_str().into());
    let evidence = &result.evaluated_rules[0].evidence;
    assert_eq!(evidence.region_bytes, 723);
    assert_eq!(evidence.region_preview, format!("{}...", "界".repeat(240)));
}

#[test]
fn manifest_parser_preserves_structured_error_sources() {
    use std::error::Error;

    let toml_error = Manifest::parse("id =").unwrap_err();
    assert!(matches!(toml_error, ManifestError::Toml(_)));
    assert!(toml_error.source().unwrap().is::<toml::de::Error>());
    let error =
        Manifest::parse(&rules_manifest("[[rules]]\nid = 'bad'\nregex = ['[']")).unwrap_err();
    assert!(
        matches!(&error, ManifestError::Rule { rule, source: ValidationError::Regex { field: "regex", .. } } if rule == "bad")
    );
    assert!(error
        .source()
        .unwrap()
        .source()
        .unwrap()
        .is::<regex::Error>());
}

#[test]
fn versioned_manifests_require_metadata_and_validate_identity() {
    let text = local_manifest("working", "active");
    assert!(matches!(
        Manifest::parse_versioned(Agent::Codex, &text),
        Err(ManifestError::MissingVersion)
    ));
    let text = format!("version = '1.0'\n{text}");
    assert!(matches!(
        Manifest::parse_versioned(Agent::Codex, &text),
        Err(ManifestError::MissingEngineVersion)
    ));
    let text = format!("min_engine_version = 3\n{text}");
    let manifest = Manifest::parse_versioned(Agent::Codex, &text).unwrap();
    assert_eq!(manifest.version().unwrap().as_str(), "1.0");
    assert!(matches!(
        Manifest::parse_for_agent(Agent::Claude, &text),
        Err(ManifestError::AgentMismatch { .. })
    ));
    assert!(
        Manifest::parse_for_agent(Agent::Claude, &format!("aliases = ['claude']\n{text}")).is_ok()
    );
    assert!(matches!(
        Manifest::parse(&text.replace("min_engine_version = 3", "min_engine_version = 4")),
        Err(ManifestError::UnsupportedEngine {
            required: 4,
            supported: 3
        })
    ));
}

#[test]
fn validation_limits_apply_across_all_rules_and_gates() {
    let many_gates = format!(
        "[[rules]]\nid = 'many'\nall = [{}]",
        vec!["{ contains = ['marker'] }"; 512].join(", ")
    );
    assert!(matches!(
        Manifest::parse(&rules_manifest(&many_gates)),
        Err(ManifestError::Rule {
            source: ValidationError::GateCount { .. },
            ..
        })
    ));
    let matchers = vec!["'marker'"; 32].join(", ");
    let many_rules = (0..33)
        .map(|i| format!("[[rules]]\nid = 'rule_{i}'\ncontains = [{matchers}]\n"))
        .collect::<String>();
    assert!(matches!(
        Manifest::parse(&rules_manifest(&many_rules)),
        Err(ManifestError::Rule {
            source: ValidationError::TotalMatchers { .. },
            ..
        })
    ));
    assert!(matches!(
        Manifest::parse(&local_manifest("working", &"界".repeat(513))),
        Err(ManifestError::Rule {
            source: ValidationError::MatcherLength { .. },
            ..
        })
    ));
    assert!(Manifest::parse(&local_manifest("working", &"界".repeat(512))).is_ok());
}

#[test]
fn screen_region_offsets_preserve_crlf_and_unicode_boundaries() {
    for (screen, spec, expected) in [
        ("界\r\n旧\r\n新\r\n", "bottom_lines(1)", "新\r\n"),
        ("界\r\n\r\n新\r\n", "bottom_non_empty_lines(1)", "新\r\n"),
        (
            "界\r\n旧\r\n新\r\n",
            "top_non_empty_lines(2)",
            "界\r\n旧\r\n",
        ),
        (
            "界\r\n› input\r\n新\r\n",
            "before_current_prompt_marker",
            "界\r\n",
        ),
        (
            "界\r\n› input\r\n新\r\n",
            "after_last_prompt_marker",
            "新\r\n",
        ),
        (
            "界\r\n───\r\n新\r\n───\r\n終\r\n",
            "prompt_box_body",
            "新\r\n",
        ),
        (
            "界\r\n───\r\n新\r\n───\r\n終\r\n",
            "after_last_horizontal_rule",
            "終\r\n",
        ),
    ] {
        assert_eq!(region(screen.into(), spec), expected, "region={spec}");
    }
}
