use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use crate::pipeline::{compress, decompress, compress_with_stats};

#[pyfunction]
fn gn_compress(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    Ok(PyBytes::new(py, &compress(data)).into())
}

#[pyfunction]
fn gn_decompress(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    decompress(data)
        .map(|d| PyBytes::new(py, &d).into())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn gn_compress_stats(py: Python, data: &[u8]) -> PyResult<(Py<PyBytes>, Py<PyDict>)> {
    let (compressed, stats) = compress_with_stats(data);
    let dict = PyDict::new(py);
    dict.set_item("input_bytes",      stats.input_bytes)?;
    dict.set_item("tokenized_bytes",  stats.tokenized_bytes)?;
    dict.set_item("compressed_bytes", stats.compressed_bytes)?;
    dict.set_item("framed_bytes",     stats.framed_bytes)?;
    dict.set_item("ratio",            stats.ratio())?;
    Ok((PyBytes::new(py, &compressed).into(), dict.into()))
}

#[pymodule]
fn glasik_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(gn_compress, m)?)?;
    m.add_function(wrap_pyfunction!(gn_decompress, m)?)?;
    m.add_function(wrap_pyfunction!(gn_compress_stats, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
