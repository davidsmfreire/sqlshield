mod select;

use std::collections::{HashMap, HashSet};

use crate::schema;

pub trait ClauseValidation {
    fn validate(
        &self,
        schema: &schema::TablesAndColumns,
        extras: &HashMap<&str, HashSet<&str>>,
    ) -> Vec<String>;
}
