use signal_executor::{
    BatchEffects, BatchPlan, CommandEffect, CommandExecutor, Executor, Lowering as LoweringTrait,
    ObserverSet, OperationEffects, OperationPlan,
};
use signal_frame::{BatchFailureReason, CommitStatus, NonEmpty, RetryClassification, SubReply};
use signal_sema::{SemaOperation, SemaOutcome, ToSemaOperation, ToSemaOutcome};
use signal_sema_upgrade::{
    Inspection, InspectionReported, Operation, Reply, ReportQuery, Reported, RequestUnimplemented,
    UnimplementedReason,
};
use thiserror::Error;

use crate::index::MigrationIndex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Inspect(Inspection),
    AttemptUpgrade(signal_sema_upgrade::Attempt),
    Report(ReportQuery),
}

impl ToSemaOperation for Command {
    fn to_sema_operation(&self) -> SemaOperation {
        match self {
            Self::Inspect(_) | Self::Report(_) => SemaOperation::Match,
            Self::AttemptUpgrade(_) => SemaOperation::Mutate,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Inspected(InspectionReported),
    Completed(signal_sema_upgrade::Completion),
    Rejected(signal_sema_upgrade::Rejection),
    Reported(Reported),
}

impl ToSemaOutcome for Effect {
    fn to_sema_outcome(&self) -> SemaOutcome {
        match self {
            Self::Inspected(_) | Self::Reported(_) => SemaOutcome::Matched,
            Self::Completed(_) => SemaOutcome::Mutated,
            Self::Rejected(_) => SemaOutcome::NoChange,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lowering;

impl LoweringTrait for Lowering {
    type Operation = Operation;
    type Reply = Reply;
    type Command = Command;
    type ComponentEffect = Effect;

    fn lower(
        &self,
        operation: &Self::Operation,
    ) -> Result<OperationPlan<Self::Command>, Self::Reply> {
        let command = match operation {
            Operation::Inspect(payload) => Command::Inspect(payload.clone()),
            Operation::AttemptUpgrade(payload) => Command::AttemptUpgrade(payload.clone()),
            Operation::Report(payload) => Command::Report(payload.clone()),
            Operation::Tap(_) | Operation::Untap(_) => {
                return Err(Reply::RequestUnimplemented(RequestUnimplemented {
                    reason: UnimplementedReason::NotBuiltYet,
                }));
            }
        };
        Ok(OperationPlan::single(command))
    }

    fn reply_from_effects(
        &self,
        _operation: &Self::Operation,
        effects: &OperationEffects<Self::Command, Self::ComponentEffect>,
    ) -> Self::Reply {
        let effect = effects
            .component_effects()
            .last()
            .expect("operation effects are non-empty");
        match effect {
            Effect::Inspected(payload) => Reply::InspectionReported(payload.clone()),
            Effect::Completed(payload) => Reply::UpgradeCompleted(payload.clone()),
            Effect::Rejected(payload) => Reply::UpgradeRejected(payload.clone()),
            Effect::Reported(payload) => Reply::Reported(payload.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Engine {
    index: MigrationIndex,
    completions: Vec<signal_sema_upgrade::Completion>,
    rejections: Vec<signal_sema_upgrade::Rejection>,
}

impl Engine {
    pub fn prototype() -> Self {
        Self {
            index: MigrationIndex::prototype(),
            completions: Vec::new(),
            rejections: Vec::new(),
        }
    }

    pub fn with_index(index: MigrationIndex) -> Self {
        Self {
            index,
            completions: Vec::new(),
            rejections: Vec::new(),
        }
    }

    pub fn executor(self) -> Executor<Lowering, Self> {
        Executor::new(Lowering, self, ObserverSet::no_op())
    }

    fn execute_command_against(
        &self,
        command: &Command,
        completions: &[signal_sema_upgrade::Completion],
        rejections: &[signal_sema_upgrade::Rejection],
    ) -> Effect {
        match command {
            Command::Inspect(Inspection::All) => Effect::Inspected(InspectionReported {
                migrations: self.index.supported_migrations(),
            }),
            Command::Inspect(Inspection::Component(component)) => {
                let migrations = self
                    .index
                    .supported_migrations()
                    .into_iter()
                    .filter(|migration| migration.component == *component)
                    .collect();
                Effect::Inspected(InspectionReported { migrations })
            }
            Command::AttemptUpgrade(attempt) => match self.index.attempt(attempt) {
                Ok(completion) => Effect::Completed(completion),
                Err(rejection) => Effect::Rejected(rejection),
            },
            Command::Report(ReportQuery::All) => Effect::Reported(Reported {
                completions: completions.to_vec(),
                rejections: rejections.to_vec(),
            }),
            Command::Report(ReportQuery::Component(component)) => {
                let completions = completions
                    .iter()
                    .filter(|completion| completion.component == *component)
                    .cloned()
                    .collect();
                let rejections = rejections
                    .iter()
                    .filter(|rejection| rejection.component == *component)
                    .cloned()
                    .collect();
                Effect::Reported(Reported {
                    completions,
                    rejections,
                })
            }
        }
    }

    fn apply_effect_to(
        effect: &Effect,
        completions: &mut Vec<signal_sema_upgrade::Completion>,
        rejections: &mut Vec<signal_sema_upgrade::Rejection>,
    ) {
        match effect {
            Effect::Completed(completion) => completions.push(completion.clone()),
            Effect::Rejected(rejection) => rejections.push(rejection.clone()),
            Effect::Inspected(_) | Effect::Reported(_) => {}
        }
    }
}

impl CommandExecutor for Engine {
    type Command = Command;
    type ComponentEffect = Effect;
    type Error = EngineError;

    async fn execute_atomic_batch(
        &mut self,
        plan: BatchPlan<Self::Command>,
    ) -> Result<BatchEffects<Self::Command, Self::ComponentEffect>, Self::Error> {
        let mut staged_completions = self.completions.clone();
        let mut staged_rejections = self.rejections.clone();
        let mut planned_operations = Vec::new();
        for operation in plan.operations() {
            let mut planned_commands = Vec::new();
            for command in operation.commands() {
                let effect =
                    self.execute_command_against(command, &staged_completions, &staged_rejections);
                Self::apply_effect_to(&effect, &mut staged_completions, &mut staged_rejections);
                planned_commands.push(CommandEffect::new(command.clone(), effect));
            }
            planned_operations.push(OperationEffects::new(
                NonEmpty::try_from_vec(planned_commands).expect("operation plans are non-empty"),
            ));
        }

        self.completions = staged_completions;
        self.rejections = staged_rejections;

        Ok(BatchEffects::new(
            NonEmpty::try_from_vec(planned_operations).expect("batch plans are non-empty"),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EngineError {
    #[error("migration engine rejected the batch")]
    Rejected,
}

impl signal_frame::BatchErrorClassification for EngineError {
    fn batch_failure_reason(&self) -> BatchFailureReason {
        BatchFailureReason::EngineRejected
    }

    fn retry_classification(&self) -> RetryClassification {
        RetryClassification::NotRetryable
    }

    fn commit_status(&self) -> CommitStatus {
        CommitStatus::NotCommitted
    }
}

pub fn first_reply(reply: signal_frame::Reply<Reply>) -> Reply {
    match reply {
        signal_frame::Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
            SubReply::Ok(payload) => payload,
            other => panic!("expected successful first reply, got {other:?}"),
        },
        other => panic!("expected accepted reply, got {other:?}"),
    }
}
