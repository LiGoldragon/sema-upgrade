use signal_sema_upgrade::{
    Attempt, Completion, ComponentName, MigrationIdentifier, Rejection, RejectionReason,
    SupportedMigration, Version,
};

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
}

impl MigrationModule {
    pub fn new(
        supported: SupportedMigration,
        run: fn(&Attempt) -> Result<ModuleResult, RejectionReason>,
    ) -> Self {
        Self { supported, run }
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
