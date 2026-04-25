use pyo3::{exceptions::PyValueError, prelude::*};

extern crate sqlshield as sqlshield_rs;

use sqlshield_rs::validation::SqlValidationError;

use std::path::Path;

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
fn validate_files(dir: String, schema_file_path: String) -> PyResult<Vec<PySqlValidationError>> {
    let validation_errors =
        sqlshield_rs::validate_files(Path::new(&dir), Path::new(&schema_file_path))
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(validation_errors
        .into_iter()
        .map(PySqlValidationError)
        .collect())
}

#[pyfunction]
fn validate_query(query: &str, schema: &str) -> PyResult<Vec<String>> {
    sqlshield_rs::validate_query(query, schema).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// A Python module implemented in Rust.
#[pymodule]
fn sqlshield(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PySqlValidationError>().unwrap();
    m.add_function(wrap_pyfunction!(validate_files, m)?)?;
    m.add_function(wrap_pyfunction!(validate_query, m)?)?;
    Ok(())
}
