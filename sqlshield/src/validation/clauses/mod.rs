//! Per-clause validators. Each SQL clause (SELECT today; INSERT/UPDATE/DELETE
//! next) implements [`ClauseValidation`] to check its references against the
//! schema plus any CTE-derived visible relations.

pub(crate) mod select;

use std::collections::{HashMap, HashSet};

use crate::schema;

pub trait ClauseValidation {
    fn validate(
        &self,
        schema: &schema::TablesAndColumns,
        extras: &HashMap<&str, HashSet<&str>>,
    ) -> Vec<String>;
}
