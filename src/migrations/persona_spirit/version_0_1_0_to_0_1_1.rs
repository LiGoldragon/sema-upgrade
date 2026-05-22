use std::path::Path;

use sema::SchemaVersion;
use sema_engine::{Assertion, Engine, EngineOpen, QueryPlan, TableDescriptor, TableName};
use signal_persona_spirit as current;
use signal_sema_upgrade::{Attempt, ComponentName, MigrationIdentifier, RejectionReason, Version};

use crate::index::{
    DatabaseMigration, DatabaseMigrationError, DatabaseMigrationResult, MigrationModule,
    ModuleResult, supported_migration,
};

pub const COMPONENT: &str = "persona-spirit";
pub const IDENTIFIER: &str = "persona-spirit-0-1-0-to-0-1-1";
pub const SOURCE: Version = Version::new(0, 1, 0);
pub const TARGET: Version = Version::new(0, 1, 1);

const SPIRIT_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1);
const RECORDS: TableName = TableName::new("records");

pub fn module() -> MigrationModule {
    MigrationModule::new(
        supported_migration(
            ComponentName::new(COMPONENT),
            SOURCE,
            TARGET,
            MigrationIdentifier::new(IDENTIFIER),
        ),
        run,
    )
    .with_database_migration(migrate_database)
}

pub fn migrate_paths(source: &Path, target: &Path) -> DatabaseMigrationResult<ModuleResult> {
    if !source.exists() {
        return Err(DatabaseMigrationError::SourceMissing(source.to_path_buf()));
    }
    if target.exists() {
        return Err(DatabaseMigrationError::TargetAlreadyExists(
            target.to_path_buf(),
        ));
    }
    if source == target {
        return Err(DatabaseMigrationError::SameSourceAndTarget(
            source.to_path_buf(),
        ));
    }

    let records = read_historical_records(source)?;
    write_current_records(target, records)
}

fn run(attempt: &Attempt) -> Result<ModuleResult, RejectionReason> {
    if attempt.component.as_str() != COMPONENT {
        return Err(RejectionReason::ComponentMismatch);
    }
    Ok(ModuleResult::unchanged())
}

fn migrate_database(request: &DatabaseMigration) -> DatabaseMigrationResult<ModuleResult> {
    migrate_paths(request.source(), request.target())
}

fn read_historical_records(
    source: &Path,
) -> DatabaseMigrationResult<Vec<historical::StoredRecord>> {
    let mut engine = Engine::open(EngineOpen::new(source, SPIRIT_SCHEMA_VERSION))
        .map_err(|error| DatabaseMigrationError::Failed(error.to_string()))?;
    let table = engine
        .register_table(TableDescriptor::<historical::StoredRecord>::new(RECORDS))
        .map_err(|error| DatabaseMigrationError::Failed(error.to_string()))?;
    let mut records = engine
        .match_records(QueryPlan::all(table))
        .map_err(|error| DatabaseMigrationError::Failed(error.to_string()))?
        .records()
        .to_vec();
    records.sort_by_key(|record| record.identifier.value());
    Ok(records)
}

fn write_current_records(
    target: &Path,
    records: Vec<historical::StoredRecord>,
) -> DatabaseMigrationResult<ModuleResult> {
    let mut engine = Engine::open(EngineOpen::new(target, SPIRIT_SCHEMA_VERSION))
        .map_err(|error| DatabaseMigrationError::Failed(error.to_string()))?;
    let table = engine
        .register_table(TableDescriptor::<current_shape::StoredRecord>::new(RECORDS))
        .map_err(|error| DatabaseMigrationError::Failed(error.to_string()))?;
    let changed_records = records.len() as u64;
    for record in records {
        engine
            .assert(Assertion::new(
                table,
                current_shape::StoredRecord::from(record),
            ))
            .map_err(|error| DatabaseMigrationError::Failed(error.to_string()))?;
    }
    Ok(ModuleResult { changed_records })
}

mod historical {
    use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
    use sema_engine::{EngineRecord, RecordKey};

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Topic(String);

    impl Topic {
        pub fn new(value: impl Into<String>) -> Self {
            Self(value.into())
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RecordIdentifier(u64);

    impl RecordIdentifier {
        pub const fn new(value: u64) -> Self {
            Self(value)
        }

        pub const fn value(self) -> u64 {
            self.0
        }
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Summary(String);

    impl Summary {
        pub fn new(value: impl Into<String>) -> Self {
            Self(value.into())
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Quote(String);

    impl Quote {
        pub fn new(value: impl Into<String>) -> Self {
            Self(value.into())
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Context(String);

    impl Context {
        pub fn new(value: impl Into<String>) -> Self {
            Self(value.into())
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Date {
        pub year: u16,
        pub month: u8,
        pub day: u8,
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Time {
        pub hour: u8,
        pub minute: u8,
        pub second: u8,
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Kind {
        Decision,
        Principle,
        Correction,
        Clarification,
        Constraint,
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Certainty {
        Maximum,
        Medium,
        Minimum,
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Entry {
        pub topic: Topic,
        pub kind: Kind,
        pub summary: Summary,
        pub context: Context,
        pub certainty: Certainty,
        pub quote: Quote,
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
    pub struct StampedEntry {
        pub entry: Entry,
        pub date: Date,
        pub time: Time,
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
    pub struct StoredRecord {
        pub identifier: RecordIdentifier,
        pub entry: StampedEntry,
    }

    impl StoredRecord {
        #[cfg(test)]
        pub fn new(identifier: RecordIdentifier, entry: StampedEntry) -> Self {
            Self { identifier, entry }
        }
    }

    impl EngineRecord for StoredRecord {
        fn record_key(&self) -> RecordKey {
            RecordKey::new(self.identifier.value().to_string())
        }
    }
}

mod current_shape {
    use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
    use sema_engine::{EngineRecord, RecordKey};
    use signal_persona_spirit::{Date, Entry, Kind, RecordIdentifier, Summary, Time, Topic};
    use signal_sema::Magnitude;

    use super::{current, historical};

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
    pub struct StampedEntry {
        pub entry: Entry,
        pub date: Date,
        pub time: Time,
    }

    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
    pub struct StoredRecord {
        pub identifier: RecordIdentifier,
        pub entry: StampedEntry,
    }

    impl From<historical::StoredRecord> for StoredRecord {
        fn from(record: historical::StoredRecord) -> Self {
            Self {
                identifier: RecordIdentifier::new(record.identifier.value()),
                entry: StampedEntry::from(record.entry),
            }
        }
    }

    impl From<historical::StampedEntry> for StampedEntry {
        fn from(entry: historical::StampedEntry) -> Self {
            Self {
                entry: Entry::from(entry.entry),
                date: Date::new(entry.date.year, entry.date.month, entry.date.day),
                time: Time::new(entry.time.hour, entry.time.minute, entry.time.second),
            }
        }
    }

    impl From<historical::Entry> for Entry {
        fn from(entry: historical::Entry) -> Self {
            Self {
                topic: Topic::new(entry.topic.as_str()),
                kind: Kind::from(entry.kind),
                summary: Summary::new(entry.summary.as_str()),
                context: current::Context::new(entry.context.as_str()),
                certainty: Magnitude::from(entry.certainty),
                quote: current::Quote::new(entry.quote.as_str()),
            }
        }
    }

    impl From<historical::Kind> for Kind {
        fn from(kind: historical::Kind) -> Self {
            match kind {
                historical::Kind::Decision => Self::Decision,
                historical::Kind::Principle => Self::Principle,
                historical::Kind::Correction => Self::Correction,
                historical::Kind::Clarification => Self::Clarification,
                historical::Kind::Constraint => Self::Constraint,
            }
        }
    }

    impl From<historical::Certainty> for Magnitude {
        fn from(certainty: historical::Certainty) -> Self {
            match certainty {
                historical::Certainty::Maximum => Self::Maximum,
                historical::Certainty::Medium => Self::Medium,
                historical::Certainty::Minimum => Self::Minimum,
            }
        }
    }

    impl EngineRecord for StoredRecord {
        fn record_key(&self) -> RecordKey {
            RecordKey::new(self.identifier.value().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use sema_engine::{Assertion, Engine, EngineOpen, QueryPlan, TableDescriptor};
    use signal_sema::Magnitude;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn migrates_historical_certainty_records_to_current_magnitude_records() {
        let directory = tempdir().expect("tempdir");
        let source = directory.path().join("spirit-v0.1.0.redb");
        let target = directory.path().join("spirit-v0.1.1.redb");
        write_historical_fixture(&source);

        let result = migrate_paths(&source, &target).expect("migration succeeds");

        assert_eq!(result.changed_records, 3);
        let records = read_current_records(&target);
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].identifier.value(), 1);
        assert_eq!(records[0].entry.entry.certainty, Magnitude::Maximum);
        assert_eq!(records[1].entry.entry.certainty, Magnitude::Medium);
        assert_eq!(records[2].entry.entry.certainty, Magnitude::Minimum);
        assert_eq!(records[0].entry.entry.topic.as_str(), "workspace");
        assert_eq!(records[0].entry.date, current::Date::new(2026, 5, 21));
        assert_eq!(records[0].entry.time, current::Time::new(17, 30, 0));
    }

    #[test]
    fn migration_rejects_existing_target_database() {
        let directory = tempdir().expect("tempdir");
        let source = directory.path().join("source.redb");
        let target = directory.path().join("target.redb");
        write_historical_fixture(&source);
        std::fs::write(&target, b"already here").expect("target marker");

        let error = migrate_paths(&source, &target).expect_err("target exists");

        assert!(matches!(
            error,
            DatabaseMigrationError::TargetAlreadyExists(_)
        ));
    }

    #[test]
    fn migration_rejects_missing_source_database() {
        let directory = tempdir().expect("tempdir");
        let source = directory.path().join("missing.redb");
        let target = directory.path().join("target.redb");

        let error = migrate_paths(&source, &target).expect_err("source missing");

        assert!(matches!(error, DatabaseMigrationError::SourceMissing(_)));
    }

    fn write_historical_fixture(path: &Path) {
        let mut engine = Engine::open(EngineOpen::new(path, SPIRIT_SCHEMA_VERSION)).expect("open");
        let table = engine
            .register_table(TableDescriptor::<historical::StoredRecord>::new(RECORDS))
            .expect("register table");
        for record in historical_records() {
            engine
                .assert(Assertion::new(table, record))
                .expect("assert record");
        }
    }

    fn historical_records() -> Vec<historical::StoredRecord> {
        vec![
            historical_record(
                1,
                historical::Kind::Decision,
                historical::Certainty::Maximum,
            ),
            historical_record(
                2,
                historical::Kind::Principle,
                historical::Certainty::Medium,
            ),
            historical_record(
                3,
                historical::Kind::Correction,
                historical::Certainty::Minimum,
            ),
        ]
    }

    fn historical_record(
        identifier: u64,
        kind: historical::Kind,
        certainty: historical::Certainty,
    ) -> historical::StoredRecord {
        historical::StoredRecord::new(
            historical::RecordIdentifier::new(identifier),
            historical::StampedEntry {
                entry: historical::Entry {
                    topic: historical::Topic::new("workspace"),
                    kind,
                    summary: historical::Summary::new(format!("summary {identifier}")),
                    context: historical::Context::new(format!("context {identifier}")),
                    certainty,
                    quote: historical::Quote::new(format!("quote {identifier}")),
                },
                date: historical::Date {
                    year: 2026,
                    month: 5,
                    day: 21,
                },
                time: historical::Time {
                    hour: 17,
                    minute: 30,
                    second: 0,
                },
            },
        )
    }

    fn read_current_records(path: &Path) -> Vec<current_shape::StoredRecord> {
        let mut engine = Engine::open(EngineOpen::new(path, SPIRIT_SCHEMA_VERSION)).expect("open");
        let table = engine
            .register_table(TableDescriptor::<current_shape::StoredRecord>::new(RECORDS))
            .expect("register table");
        engine
            .match_records(QueryPlan::all(table))
            .expect("match")
            .records()
            .to_vec()
    }
}
