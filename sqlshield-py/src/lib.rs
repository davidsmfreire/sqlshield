use pyo3::prelude::*;

extern crate sqlshield as sqlshield_rs;

use sqlshield_rs::validation::SqlValidationError;

use std::path::PathBuf;

#[pyclass]
struct PySqlValidationError(SqlValidationError);

#[pymethods]
impl PySqlValidationError {
    fn __str__(&self) -> String {
        self.0.to_string()
    }

    #[getter]
    fn location(&self) -> PyResult<&str> {
        Ok(&self.0.location)
    }

    #[getter]
    fn description(&self) -> PyResult<&str> {
        Ok(&self.0.description)
    }
}

#[pyfunction]
fn validate(dir: String, schema_file_path: String) -> PyResult<Vec<PySqlValidationError>> {
    Ok(
        sqlshield_rs::validate(&PathBuf::from(dir), &PathBuf::from(schema_file_path))
            .into_iter()
            .map(|err| PySqlValidationError(err))
            .collect::<Vec<PySqlValidationError>>(),
    )
}

/// A Python module implemented in Rust.
#[pymodule]
fn sqlshield(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PySqlValidationError>().unwrap();
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    Ok(())
}
