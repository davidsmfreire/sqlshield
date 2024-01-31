mod select;

use crate::schema;

pub trait ClauseValidation {
    fn validate(&self, schema: &schema::TablesAndColumns) -> Vec<String>;
}
