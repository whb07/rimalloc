
use super::*;

#[test]
fn guarded_config_rejects_inverted_size_bounds() {
    assert!(
        GuardedConfig::new(
            GuardedSizeMin::new(64),
            GuardedSizeMax::new(63),
            GuardedSampleRate::new(1),
            GuardedSampleCount::new(1),
        )
        .is_none()
    );
}

#[test]
fn guarded_config_exposes_typed_sampling_fields() {
    let config = GuardedConfig::new(
        GuardedSizeMin::new(64),
        GuardedSizeMax::new(128),
        GuardedSampleRate::new(7),
        GuardedSampleCount::new(2),
    )
    .unwrap();

    assert!(config.contains_size(64));
    assert!(config.contains_size(128));
    assert!(!config.contains_size(129));
    assert_eq!(config.sample_rate().get(), 7);
    assert!(!config.sample_rate().is_zero());
    assert_eq!(config.sample_count().decrement_wrapping().get(), 1);
}

#[test]
fn guarded_decision_decrements_and_skips_until_count_expires() {
    let decision = GuardedConfig::new(
        GuardedSizeMin::new(64),
        GuardedSizeMax::new(128),
        GuardedSampleRate::new(7),
        GuardedSampleCount::new(3),
    )
    .unwrap()
    .decide(96);

    assert_eq!(
        decision,
        GuardedDecision::SkipAndUpdate {
            next_sample_count: GuardedSampleCount::new(2),
        }
    );
    assert!(!decision.is_guarded());
}

#[test]
fn guarded_decision_uses_guarded_when_count_expires_in_range() {
    let decision = GuardedConfig::new(
        GuardedSizeMin::new(64),
        GuardedSizeMax::new(128),
        GuardedSampleRate::new(7),
        GuardedSampleCount::new(1),
    )
    .unwrap()
    .decide(96);

    assert_eq!(
        decision,
        GuardedDecision::UseGuarded {
            next_sample_count: GuardedSampleCount::new(7),
        }
    );
    assert_eq!(decision.next_sample_count().unwrap().get(), 7);
}

#[test]
fn guarded_decision_resets_to_one_when_count_expires_out_of_range() {
    let decision = GuardedConfig::new(
        GuardedSizeMin::new(64),
        GuardedSizeMax::new(128),
        GuardedSampleRate::new(7),
        GuardedSampleCount::new(1),
    )
    .unwrap()
    .decide(129);

    assert_eq!(
        decision,
        GuardedDecision::SkipAndUpdate {
            next_sample_count: GuardedSampleCount::new(1),
        }
    );
    assert!(!decision.is_guarded());
}

#[test]
fn guarded_decision_leaves_count_unchanged_when_disabled_out_of_range() {
    let decision = GuardedConfig::new(
        GuardedSizeMin::new(64),
        GuardedSizeMax::new(128),
        GuardedSampleRate::new(0),
        GuardedSampleCount::new(1),
    )
    .unwrap()
    .decide(129);

    assert_eq!(decision, GuardedDecision::SkipUnchanged);
    assert_eq!(decision.next_sample_count(), None);
    assert!(!decision.is_guarded());
}
