//! Retired prototype crate for Sema schema upgrades.
//!
//! The migration catalogue, executor, and handover driver moved to the
//! `upgrade` triad during the `/318` upgrade merger. This crate remains
//! only as an explicit breadcrumb for old pins and documentation links.

pub const RETIRED_BY_CRATE: &str = "upgrade";
pub const RETIRED_BY_REPOSITORY: &str = "https://github.com/LiGoldragon/upgrade";

pub fn retired_by_crate() -> &'static str {
    RETIRED_BY_CRATE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_crate_points_to_upgrade() {
        assert_eq!(retired_by_crate(), "upgrade");
        assert_eq!(
            RETIRED_BY_REPOSITORY,
            "https://github.com/LiGoldragon/upgrade"
        );
    }
}
