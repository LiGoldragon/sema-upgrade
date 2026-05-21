use signal_sema_upgrade::{Attempt, ComponentName, MigrationIdentifier, RejectionReason, Version};

use crate::index::{MigrationModule, ModuleResult, supported_migration};

pub const COMPONENT: &str = "persona-spirit";
pub const IDENTIFIER: &str = "persona-spirit-0-1-0-to-0-1-1";
pub const SOURCE: Version = Version::new(0, 1, 0);
pub const TARGET: Version = Version::new(0, 1, 1);

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
}

fn run(attempt: &Attempt) -> Result<ModuleResult, RejectionReason> {
    if attempt.component.as_str() != COMPONENT {
        return Err(RejectionReason::ComponentMismatch);
    }
    Ok(ModuleResult::unchanged())
}
