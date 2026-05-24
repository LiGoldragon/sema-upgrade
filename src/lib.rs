//! Runtime prototype for Sema schema upgrades.
//!
//! The library wires the ordinary `signal-sema-upgrade` contract through
//! `signal-executor`. The first migration is a compile-time module for
//! `persona-spirit` `0.1.0` to `0.1.1`.

pub mod execution;
pub mod index;
pub mod migrations;

pub use execution::{Command, Effect, Engine, EngineError, Lowering, first_reply};
pub use index::{
    DatabaseMigration, DatabaseMigrationError, DatabaseMigrationResult, MigrationIndex,
    MigrationModule, ModuleResult,
};
