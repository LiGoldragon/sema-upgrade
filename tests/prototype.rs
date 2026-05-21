use sema_upgrade::{Command, Effect, Engine, MigrationIndex, first_reply};
use signal_executor::Executor;
use signal_frame::{AcceptedOutcome, RequestBuilder, RequestPayload, SubReply};
use signal_sema::{SemaOperation, SemaOutcome, ToSemaOperation, ToSemaOutcome};
use signal_sema_upgrade::{
    Attempt, ComponentName, Inspection, Operation, RejectionReason, Reply, ReportQuery, Version,
};

fn attempt(source: Version, target: Version) -> Attempt {
    Attempt {
        component: ComponentName::new("persona-spirit"),
        source,
        target,
    }
}

fn supported_attempt() -> Attempt {
    attempt(Version::new(0, 1, 0), Version::new(0, 1, 1))
}

fn unsupported_attempt() -> Attempt {
    attempt(Version::new(0, 1, 0), Version::new(0, 1, 2))
}

#[test]
fn module_index_names_persona_spirit_version_upgrade() {
    let index = MigrationIndex::prototype();
    let migrations = index.supported_migrations();

    assert_eq!(migrations.len(), 1);
    assert_eq!(migrations[0].component.as_str(), "persona-spirit");
    assert_eq!(migrations[0].source, Version::new(0, 1, 0));
    assert_eq!(migrations[0].target, Version::new(0, 1, 1));
    assert_eq!(
        migrations[0].identifier.as_str(),
        "persona-spirit-0-1-0-to-0-1-1"
    );
}

#[test]
fn commands_project_to_sema_classification() {
    let inspect = Command::Inspect(Inspection::All);
    let attempt = Command::AttemptUpgrade(supported_attempt());
    let completed = Effect::Completed(signal_sema_upgrade::Completion {
        component: ComponentName::new("persona-spirit"),
        source: Version::new(0, 1, 0),
        target: Version::new(0, 1, 1),
        migration: signal_sema_upgrade::MigrationIdentifier::new("persona-spirit-0-1-0-to-0-1-1"),
        changed_records: 0,
    });

    assert_eq!(inspect.to_sema_operation(), SemaOperation::Match);
    assert_eq!(attempt.to_sema_operation(), SemaOperation::Mutate);
    assert_eq!(completed.to_sema_outcome(), SemaOutcome::Mutated);
}

#[tokio::test]
async fn supported_upgrade_runs_through_signal_executor() {
    let mut executor: Executor<_, _> = Engine::prototype().executor();

    let reply = executor
        .execute(Operation::AttemptUpgrade(supported_attempt()).into_request())
        .await;

    let Reply::UpgradeCompleted(completion) = first_reply(reply) else {
        panic!("expected UpgradeCompleted");
    };
    assert_eq!(completion.component.as_str(), "persona-spirit");
    assert_eq!(completion.source, Version::new(0, 1, 0));
    assert_eq!(completion.target, Version::new(0, 1, 1));
    assert_eq!(completion.changed_records, 0);
}

#[tokio::test]
async fn unsupported_upgrade_rejects_as_typed_contract_reply() {
    let mut executor = Engine::prototype().executor();

    let reply = executor
        .execute(Operation::AttemptUpgrade(unsupported_attempt()).into_request())
        .await;

    let Reply::UpgradeRejected(rejection) = first_reply(reply) else {
        panic!("expected UpgradeRejected");
    };
    assert_eq!(rejection.component.as_str(), "persona-spirit");
    assert_eq!(rejection.target, Version::new(0, 1, 2));
    assert_eq!(rejection.reason, RejectionReason::UnsupportedMigration);
}

#[tokio::test]
async fn multi_operation_request_is_atomic_unit_for_executor() {
    let mut executor = Engine::prototype().executor();
    let request = RequestBuilder::new()
        .with(Operation::AttemptUpgrade(supported_attempt()))
        .with(Operation::Report(ReportQuery::All))
        .build()
        .expect("non-empty request");

    let reply = executor.execute(request).await;

    let signal_frame::Reply::Accepted {
        outcome,
        per_operation,
    } = reply
    else {
        panic!("expected accepted reply");
    };
    assert_eq!(outcome, AcceptedOutcome::Committed);

    let (first, tail) = per_operation.into_head_and_tail();
    assert!(matches!(first, SubReply::Ok(Reply::UpgradeCompleted(_))));
    assert_eq!(tail.len(), 1);
    let SubReply::Ok(Reply::Reported(report)) = &tail[0] else {
        panic!("expected report reply");
    };
    assert_eq!(report.completions.len(), 1);
    assert!(report.rejections.is_empty());
}
