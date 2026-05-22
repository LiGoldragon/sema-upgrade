use sema_upgrade::{
    ActiveVersion, EndpointState, HandoverError, MirrorDecision, PrototypeHandover,
};
use signal_sema::Magnitude;
use signal_version_handover::{DivergenceReason, HandoverRejectionReason};
use version_projection::{ProjectionError, RecordKind, VersionProjection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyCertainty {
    Maximum,
    Medium,
    Minimum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LegacySignalEntry {
    certainty: LegacyCertainty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CurrentSignalEntry {
    certainty: Magnitude,
}

struct LegacyToCurrent;
struct CurrentToLegacy;

impl VersionProjection<LegacySignalEntry, CurrentSignalEntry> for LegacyToCurrent {
    type Error = ProjectionError;

    fn project(source: LegacySignalEntry) -> Result<CurrentSignalEntry, Self::Error> {
        let certainty = match source.certainty {
            LegacyCertainty::Maximum => Magnitude::Maximum,
            LegacyCertainty::Medium => Magnitude::Medium,
            LegacyCertainty::Minimum => Magnitude::Minimum,
        };
        Ok(CurrentSignalEntry { certainty })
    }
}

impl VersionProjection<CurrentSignalEntry, LegacySignalEntry> for CurrentToLegacy {
    type Error = ProjectionError;

    fn project(source: CurrentSignalEntry) -> Result<LegacySignalEntry, Self::Error> {
        let certainty = match source.certainty {
            Magnitude::Maximum => LegacyCertainty::Maximum,
            Magnitude::Medium => LegacyCertainty::Medium,
            Magnitude::Minimum => LegacyCertainty::Minimum,
            _ => {
                return Err(ProjectionError::NotRepresentable {
                    source_type: "CurrentSignalEntry".to_string(),
                    target_type: "LegacySignalEntry".to_string(),
                });
            }
        };
        Ok(LegacySignalEntry { certainty })
    }
}

#[test]
fn handover_flips_active_endpoint_and_leaves_current_private_upgrade_only() {
    let mut handover = PrototypeHandover::for_spirit_0_1_0_to_0_1_1();

    handover.run_atomic_handover().expect("handover");

    assert_eq!(handover.active(), ActiveVersion::Next);
    assert_eq!(
        handover.current().state(),
        EndpointState::PrivateUpgradeOnly
    );
    assert_eq!(handover.next().state(), EndpointState::Public);
}

#[test]
fn handover_rejects_if_current_marker_moves_before_acceptance() {
    let mut handover = PrototypeHandover::for_spirit_0_1_0_to_0_1_1();

    let error = handover
        .run_with_stale_marker_for_test()
        .expect_err("marker advanced");

    let HandoverError::Rejected(rejection) = error else {
        panic!("expected rejection");
    };
    assert_eq!(
        rejection.reason,
        HandoverRejectionReason::CommitSequenceAdvanced
    );
    assert_eq!(handover.active(), ActiveVersion::Current);
}

#[test]
fn representable_projection_mirrors_to_peer_version() {
    let mut handover = PrototypeHandover::for_spirit_0_1_0_to_0_1_1();
    handover.run_atomic_handover().expect("handover");

    let decision = handover.mirror_with_projection::<CurrentToLegacy, _, _>(
        CurrentSignalEntry {
            certainty: Magnitude::Maximum,
        },
        vec![1, 2, 3],
        RecordKind::new("Entry"),
    );

    let MirrorDecision::Mirrored {
        target,
        acknowledgement,
    } = decision
    else {
        panic!("expected mirrored");
    };
    assert_eq!(target.certainty, LegacyCertainty::Maximum);
    assert_eq!(acknowledgement.write_counter, 1);
    assert!(handover.divergences().is_empty());
}

#[test]
fn widened_signal_type_records_divergence_for_old_peer() {
    let mut handover = PrototypeHandover::for_spirit_0_1_0_to_0_1_1();
    handover.run_atomic_handover().expect("handover");

    let decision = handover.mirror_with_projection::<CurrentToLegacy, _, _>(
        CurrentSignalEntry {
            certainty: Magnitude::High,
        },
        vec![8, 13, 21],
        RecordKind::new("Entry"),
    );

    let MirrorDecision::Diverged { acknowledgement } = decision else {
        panic!("expected divergence");
    };
    assert_eq!(acknowledgement.divergence_identifier, 1);
    assert_eq!(handover.divergences().len(), 1);
    assert_eq!(
        handover.divergences()[0].reason,
        DivergenceReason::NotRepresentable
    );
}

#[test]
fn legacy_entry_projection_upgrades_cleanly() {
    let current = LegacyToCurrent::project(LegacySignalEntry {
        certainty: LegacyCertainty::Medium,
    })
    .expect("projection");

    assert_eq!(current.certainty, Magnitude::Medium);
}
