use std::path::PathBuf;

use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode, NotaEnum, NotaRecord, Path};
use sema_upgrade::{DatabaseMigration, DatabaseMigrationError, MigrationIndex};
use signal_sema_upgrade::{Attempt, Completion, Rejection, RejectionReason, Reply};

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
struct TemporaryAttempt {
    source: Path,
    target: Path,
    attempt: Attempt,
}

#[derive(NotaEnum, Debug, Clone, PartialEq, Eq)]
enum TemporaryCommand {
    Attempt(TemporaryAttempt),
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let input = read_single_argument()?;
    let command = decode_command(&input)?;
    let reply = execute(command);
    print_reply(&reply)?;
    if matches!(reply, Reply::UpgradeRejected(_)) {
        return Err("migration rejected".to_string());
    }
    Ok(())
}

fn read_single_argument() -> Result<String, String> {
    let mut arguments = std::env::args().skip(1);
    let input = arguments
        .next()
        .ok_or_else(|| "expected exactly one NOTA argument or NOTA file path".to_string())?;
    if arguments.next().is_some() {
        return Err("expected exactly one NOTA argument or NOTA file path".to_string());
    }
    if input.trim_start().starts_with('(') {
        Ok(input)
    } else {
        std::fs::read_to_string(&input).map_err(|error| format!("failed to read {input}: {error}"))
    }
}

fn decode_command(input: &str) -> Result<TemporaryCommand, String> {
    let mut decoder = Decoder::new(input);
    let command = TemporaryCommand::decode(&mut decoder).map_err(|error| error.to_string())?;
    if let Some(token) = decoder.peek_token().map_err(|error| error.to_string())? {
        return Err(format!("expected end of input, got {token:?}"));
    }
    Ok(command)
}

fn execute(command: TemporaryCommand) -> Reply {
    match command {
        TemporaryCommand::Attempt(attempt) => {
            let migration = DatabaseMigration::new(
                attempt.attempt.clone(),
                PathBuf::from(attempt.source.as_str()),
                PathBuf::from(attempt.target.as_str()),
            );
            match MigrationIndex::prototype().migrate_database(&migration) {
                Ok(completion) => Reply::UpgradeCompleted(completion),
                Err(error) => Reply::UpgradeRejected(rejection_from_error(&attempt.attempt, error)),
            }
        }
    }
}

fn rejection_from_error(attempt: &Attempt, error: DatabaseMigrationError) -> Rejection {
    match error {
        DatabaseMigrationError::Rejected(rejection) => rejection,
        DatabaseMigrationError::UnsupportedMigration => Rejection {
            component: attempt.component.clone(),
            source: attempt.source,
            target: attempt.target,
            reason: RejectionReason::UnsupportedMigration,
        },
        DatabaseMigrationError::NoDatabaseMigration { .. }
        | DatabaseMigrationError::SourceMissing(_)
        | DatabaseMigrationError::TargetAlreadyExists(_)
        | DatabaseMigrationError::SameSourceAndTarget(_)
        | DatabaseMigrationError::Failed(_) => Rejection {
            component: attempt.component.clone(),
            source: attempt.source,
            target: attempt.target,
            reason: RejectionReason::MigrationFailed,
        },
    }
}

fn print_reply(reply: &Reply) -> Result<(), String> {
    let mut encoder = Encoder::new();
    reply
        .encode(&mut encoder)
        .map_err(|error| error.to_string())?;
    println!("{}", encoder.into_string());
    Ok(())
}

#[allow(dead_code)]
fn _assert_completion_is_nota(_: &Completion) {}
