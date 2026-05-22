use std::path::{Path, PathBuf};

use signal_sema_upgrade::{
    Attempt, Completion, ComponentName, MigrationIdentifier, Rejection, RejectionReason,
    SupportedMigration, Version,
};
use thiserror::Error;

use crate::migrations;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleResult {
    pub changed_records: u64,
}

impl ModuleResult {
    pub const fn unchanged() -> Self {
        Self { changed_records: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct MigrationModule {
    supported: SupportedMigration,
    run: fn(&Attempt) -> Result<ModuleResult, RejectionReason>,
    migrate_database: Option<fn(&DatabaseMigration) -> DatabaseMigrationResult<ModuleResult>>,
}

impl MigrationModule {
    pub fn new(
        supported: SupportedMigration,
        run: fn(&Attempt) -> Result<ModuleResult, RejectionReason>,
    ) -> Self {
        Self {
            supported,
            run,
            migrate_database: None,
        }
    }

    pub fn with_database_migration(
        mut self,
        migrate_database: fn(&DatabaseMigration) -> DatabaseMigrationResult<ModuleResult>,
    ) -> Self {
        self.migrate_database = Some(migrate_database);
        self
    }

    pub fn supported(&self) -> &SupportedMigration {
        &self.supported
    }

    pub fn matches(&self, attempt: &Attempt) -> bool {
        self.supported.component == attempt.component
            && self.supported.source == attempt.source
            && self.supported.target == attempt.target
    }

    pub fn run(&self, attempt: &Attempt) -> Result<Completion, Rejection> {
        if !self.matches(attempt) {
            return Err(rejection(attempt, RejectionReason::ComponentMismatch));
        }
        let result = (self.run)(attempt).map_err(|reason| rejection(attempt, reason))?;
        Ok(Completion {
            component: attempt.component.clone(),
            source: attempt.source,
            target: attempt.target,
            migration: self.supported.identifier.clone(),
            changed_records: result.changed_records,
        })
    }

    pub fn migrate_database(
        &self,
        request: &DatabaseMigration,
    ) -> DatabaseMigrationResult<Completion> {
        if !self.matches(request.attempt()) {
            return Err(DatabaseMigrationError::Rejected(rejection(
                request.attempt(),
                RejectionReason::ComponentMismatch,
            )));
        }
        let migrate_database =
            self.migrate_database
                .ok_or(DatabaseMigrationError::NoDatabaseMigration {
                    migration: self.supported.identifier.clone(),
                })?;
        let result = migrate_database(request)?;
        Ok(Completion {
            component: request.attempt.component.clone(),
            source: request.attempt.source,
            target: request.attempt.target,
            migration: self.supported.identifier.clone(),
            changed_records: result.changed_records,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseMigration {
    attempt: Attempt,
    source: PathBuf,
    target: PathBuf,
}

impl DatabaseMigration {
    pub fn new(attempt: Attempt, source: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        Self {
            attempt,
            source: source.into(),
            target: target.into(),
        }
    }

    pub fn attempt(&self) -> &Attempt {
        &self.attempt
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn target(&self) -> &Path {
        &self.target
    }
}

pub type DatabaseMigrationResult<T> = Result<T, DatabaseMigrationError>;

#[derive(Debug, Error)]
pub enum DatabaseMigrationError {
    #[error("unsupported migration")]
    UnsupportedMigration,
    #[error("migration rejected: {0:?}")]
    Rejected(Rejection),
    #[error("migration {migration:?} has no database migration implementation")]
    NoDatabaseMigration { migration: MigrationIdentifier },
    #[error("source database does not exist: {0}")]
    SourceMissing(PathBuf),
    #[error("target database already exists: {0}")]
    TargetAlreadyExists(PathBuf),
    #[error("source and target database paths must differ: {0}")]
    SameSourceAndTarget(PathBuf),
    #[error("database migration failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct MigrationIndex {
    modules: Vec<MigrationModule>,
}

impl MigrationIndex {
    pub fn new(modules: Vec<MigrationModule>) -> Self {
        Self { modules }
    }

    pub fn prototype() -> Self {
        Self::new(vec![
            migrations::persona_spirit::version_0_1_0_to_0_1_1::module(),
        ])
    }

    pub fn modules(&self) -> &[MigrationModule] {
        &self.modules
    }

    pub fn supported_migrations(&self) -> Vec<SupportedMigration> {
        self.modules
            .iter()
            .map(|module| module.supported().clone())
            .collect()
    }

    pub fn find(&self, attempt: &Attempt) -> Option<&MigrationModule> {
        self.modules.iter().find(|module| module.matches(attempt))
    }

    pub fn attempt(&self, attempt: &Attempt) -> Result<Completion, Rejection> {
        self.find(attempt)
            .ok_or_else(|| rejection(attempt, RejectionReason::UnsupportedMigration))
            .and_then(|module| module.run(attempt))
    }

    pub fn migrate_database(
        &self,
        request: &DatabaseMigration,
    ) -> DatabaseMigrationResult<Completion> {
        self.find(request.attempt())
            .ok_or(DatabaseMigrationError::UnsupportedMigration)
            .and_then(|module| module.migrate_database(request))
    }
}

pub fn supported_migration(
    component: ComponentName,
    source: Version,
    target: Version,
    identifier: MigrationIdentifier,
) -> SupportedMigration {
    SupportedMigration {
        component,
        source,
        target,
        identifier,
    }
}

fn rejection(attempt: &Attempt, reason: RejectionReason) -> Rejection {
    Rejection {
        component: attempt.component.clone(),
        source: attempt.source,
        target: attempt.target,
        reason,
    }
}
