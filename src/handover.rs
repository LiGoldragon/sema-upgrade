use signal_version_handover::{
    CompletionReport, Date, DivergenceAcknowledgement, DivergencePayload, DivergenceReason,
    HandoverAcceptance, HandoverFinalization, HandoverMarker, HandoverRejection,
    HandoverRejectionReason, MarkerRequest, MirrorAcknowledgement, Operation, ReadinessReport,
    RecoveryResult, Reply, Time,
};
use thiserror::Error;
use version_projection::{
    ComponentName, ContractVersion, ProjectionError, RecordKind, VersionProjection,
};

/// Which daemon version currently owns the public working and owner sockets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveVersion {
    Current,
    Next,
}

/// Prototype endpoint state for the handover socket relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointState {
    Public,
    Handover,
    PrivateUpgradeOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrototypeEndpoint {
    marker: HandoverMarker,
    state: EndpointState,
}

impl PrototypeEndpoint {
    pub fn new(marker: HandoverMarker) -> Self {
        Self {
            marker,
            state: EndpointState::Public,
        }
    }

    pub fn marker(&self) -> &HandoverMarker {
        &self.marker
    }

    pub fn state(&self) -> EndpointState {
        self.state
    }

    fn handle_current_operation(&mut self, operation: Operation) -> Reply {
        match operation {
            Operation::AskHandoverMarker(request) => self.reply_marker(request),
            Operation::ReadyToHandover(report) => self.accept_ready(report),
            Operation::HandoverCompleted(report) => self.finalize(report),
            Operation::Mirror(payload) => {
                self.marker.write_counter += 1;
                Reply::MirrorAcknowledged(MirrorAcknowledgement {
                    component: payload.component,
                    write_counter: self.marker.write_counter,
                })
            }
            Operation::Divergence(payload) => {
                self.marker.write_counter += 1;
                Reply::DivergenceAcknowledged(DivergenceAcknowledgement {
                    component: payload.component,
                    divergence_identifier: self.marker.write_counter,
                })
            }
            Operation::RecoverFromFailure(request) => Reply::RecoveryCompleted(RecoveryResult {
                component: request.component,
                recovered: false,
            }),
        }
    }

    fn reply_marker(&self, request: MarkerRequest) -> Reply {
        if request.component == self.marker.component {
            Reply::HandoverMarker(self.marker.clone())
        } else {
            Reply::HandoverRejected(HandoverRejection {
                component: request.component,
                reason: HandoverRejectionReason::SchemaMismatch,
            })
        }
    }

    fn accept_ready(&mut self, report: ReadinessReport) -> Reply {
        if report.component != self.marker.component {
            return Reply::HandoverRejected(HandoverRejection {
                component: report.component,
                reason: HandoverRejectionReason::SchemaMismatch,
            });
        }
        if report.source_marker.commit_sequence != self.marker.commit_sequence
            || report.source_marker.write_counter != self.marker.write_counter
        {
            return Reply::HandoverRejected(HandoverRejection {
                component: report.component,
                reason: HandoverRejectionReason::CommitSequenceAdvanced,
            });
        }
        self.state = EndpointState::Handover;
        Reply::HandoverAccepted(HandoverAcceptance {
            accepted_marker: self.marker.clone(),
        })
    }

    fn finalize(&mut self, report: CompletionReport) -> Reply {
        if report.component != self.marker.component {
            return Reply::HandoverRejected(HandoverRejection {
                component: report.component,
                reason: HandoverRejectionReason::SchemaMismatch,
            });
        }
        self.state = EndpointState::PrivateUpgradeOnly;
        Reply::HandoverFinalized(HandoverFinalization {
            finalized_marker: report.accepted_marker,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PrototypeHandover {
    current: PrototypeEndpoint,
    next: PrototypeEndpoint,
    active: ActiveVersion,
    divergences: Vec<DivergencePayload>,
}

impl PrototypeHandover {
    pub fn new(current: PrototypeEndpoint, next: PrototypeEndpoint) -> Self {
        Self {
            current,
            next,
            active: ActiveVersion::Current,
            divergences: Vec::new(),
        }
    }

    pub fn for_spirit_0_1_0_to_0_1_1() -> Self {
        let component = ComponentName::new("persona-spirit");
        Self::new(
            PrototypeEndpoint::new(marker(component.clone(), ContractVersion::new([1; 32]))),
            PrototypeEndpoint::new(marker(component, ContractVersion::new([2; 32]))),
        )
    }

    pub fn active(&self) -> ActiveVersion {
        self.active
    }

    pub fn current(&self) -> &PrototypeEndpoint {
        &self.current
    }

    pub fn next(&self) -> &PrototypeEndpoint {
        &self.next
    }

    pub fn divergences(&self) -> &[DivergencePayload] {
        &self.divergences
    }

    pub fn run_atomic_handover(&mut self) -> Result<(), HandoverError> {
        let component = self.current.marker.component.clone();
        let marker = match self
            .current
            .handle_current_operation(Operation::AskHandoverMarker(MarkerRequest {
                component: component.clone(),
            })) {
            Reply::HandoverMarker(marker) => marker,
            Reply::HandoverRejected(rejection) => return Err(HandoverError::Rejected(rejection)),
            other => return Err(HandoverError::UnexpectedReply(reply_name(&other))),
        };

        match self
            .current
            .handle_current_operation(Operation::ReadyToHandover(ReadinessReport {
                component: component.clone(),
                source_marker: marker.clone(),
            })) {
            Reply::HandoverAccepted(_) => {}
            Reply::HandoverRejected(rejection) => return Err(HandoverError::Rejected(rejection)),
            other => return Err(HandoverError::UnexpectedReply(reply_name(&other))),
        }

        self.next.state = EndpointState::Public;
        self.active = ActiveVersion::Next;

        match self
            .current
            .handle_current_operation(Operation::HandoverCompleted(CompletionReport {
                component,
                accepted_marker: marker,
            })) {
            Reply::HandoverFinalized(_) => Ok(()),
            Reply::HandoverRejected(rejection) => Err(HandoverError::Rejected(rejection)),
            other => Err(HandoverError::UnexpectedReply(reply_name(&other))),
        }
    }

    pub fn run_with_stale_marker_for_test(&mut self) -> Result<(), HandoverError> {
        let component = self.current.marker.component.clone();
        let mut stale_marker = self.current.marker.clone();
        self.current.marker.write_counter += 1;
        stale_marker.write_counter = stale_marker.write_counter.saturating_sub(1);

        match self
            .current
            .handle_current_operation(Operation::ReadyToHandover(ReadinessReport {
                component,
                source_marker: stale_marker,
            })) {
            Reply::HandoverAccepted(_) => Ok(()),
            Reply::HandoverRejected(rejection) => Err(HandoverError::Rejected(rejection)),
            other => Err(HandoverError::UnexpectedReply(reply_name(&other))),
        }
    }

    pub fn mirror_with_projection<Projection, Source, Target>(
        &mut self,
        source: Source,
        payload: Vec<u8>,
        kind: RecordKind,
    ) -> MirrorDecision<Target>
    where
        Projection: VersionProjection<Source, Target, Error = ProjectionError>,
    {
        let component = self.current.marker.component.clone();
        let source_version = self.next.marker.schema_hash;
        let target_version = self.current.marker.schema_hash;

        match Projection::project(source) {
            Ok(target) => {
                let acknowledgement = match self.current.handle_current_operation(
                    Operation::Mirror(signal_version_handover::MirrorPayload {
                        component,
                        source_version,
                        target_version,
                        kind,
                        payload,
                    }),
                ) {
                    Reply::MirrorAcknowledged(acknowledgement) => acknowledgement,
                    other => panic!("unexpected mirror reply: {}", reply_name(&other)),
                };
                MirrorDecision::Mirrored {
                    target,
                    acknowledgement,
                }
            }
            Err(error) => {
                let divergence = DivergencePayload {
                    component,
                    source_version,
                    target_version,
                    reason: divergence_reason(&error),
                    kind,
                    payload,
                };
                let acknowledgement = match self
                    .current
                    .handle_current_operation(Operation::Divergence(divergence.clone()))
                {
                    Reply::DivergenceAcknowledged(acknowledgement) => acknowledgement,
                    other => panic!("unexpected divergence reply: {}", reply_name(&other)),
                };
                self.divergences.push(divergence);
                MirrorDecision::Diverged { acknowledgement }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirrorDecision<Target> {
    Mirrored {
        target: Target,
        acknowledgement: MirrorAcknowledgement,
    },
    Diverged {
        acknowledgement: DivergenceAcknowledgement,
    },
}

#[derive(Debug, Error)]
pub enum HandoverError {
    #[error("handover rejected: {0:?}")]
    Rejected(HandoverRejection),
    #[error("unexpected handover reply: {0}")]
    UnexpectedReply(&'static str),
}

fn marker(component: ComponentName, schema_hash: ContractVersion) -> HandoverMarker {
    HandoverMarker {
        component,
        schema_hash,
        commit_sequence: 0,
        write_counter: 0,
        last_record_identifier: None,
        recorded_at_date: Date::new(2026, 5, 22),
        recorded_at_time: Time::new(12, 0, 0),
    }
}

fn divergence_reason(error: &ProjectionError) -> DivergenceReason {
    match error {
        ProjectionError::NotRepresentable { .. } => DivergenceReason::NotRepresentable,
        ProjectionError::TransformFailed(_) => DivergenceReason::TargetRejected,
        ProjectionError::DirectionNotImplemented => DivergenceReason::TargetUnavailable,
    }
}

fn reply_name(reply: &Reply) -> &'static str {
    match reply {
        Reply::HandoverMarker(_) => "HandoverMarker",
        Reply::HandoverAccepted(_) => "HandoverAccepted",
        Reply::HandoverFinalized(_) => "HandoverFinalized",
        Reply::MirrorAcknowledged(_) => "MirrorAcknowledged",
        Reply::DivergenceAcknowledged(_) => "DivergenceAcknowledged",
        Reply::RecoveryCompleted(_) => "RecoveryCompleted",
        Reply::HandoverRejected(_) => "HandoverRejected",
    }
}
