use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn ators(m: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}
