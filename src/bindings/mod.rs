use crate::pipeline::{compress, compress_batch_with_stats, compress_with_stats, decompress};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

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
    dict.set_item("input_bytes", stats.input_bytes)?;
    dict.set_item("tokenized_bytes", stats.tokenized_bytes)?;
    dict.set_item("compressed_bytes", stats.compressed_bytes)?;
    dict.set_item("framed_bytes", stats.framed_bytes)?;
    dict.set_item("ratio", stats.ratio())?;
    Ok((PyBytes::new(py, &compressed).into(), dict.into()))
}

#[pyfunction]
fn gn_compress_batch(py: Python, messages: Vec<Vec<u8>>) -> PyResult<Py<PyList>> {
    let refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let (frames, _) = compress_batch_with_stats(&refs);
    let list = PyList::new(py, frames.iter().map(|f| PyBytes::new(py, f)))?;
    Ok(list.into())
}

#[pyfunction]
fn gn_compress_batch_stats(
    py: Python,
    messages: Vec<Vec<u8>>,
) -> PyResult<(Py<PyList>, Py<PyDict>)> {
    let refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let (frames, stats) = compress_batch_with_stats(&refs);
    let list = PyList::new(py, frames.iter().map(|f| PyBytes::new(py, f)))?;
    let dict = PyDict::new(py);
    dict.set_item("input_bytes", stats.input_bytes)?;
    dict.set_item("tokenized_bytes", stats.tokenized_bytes)?;
    dict.set_item("compressed_bytes", stats.compressed_bytes)?;
    dict.set_item("framed_bytes", stats.framed_bytes)?;
    dict.set_item("ratio", stats.ratio())?;
    Ok((list.into(), dict.into()))
}

// ── Sliding window API ────────────────────────────────────────────────────────
/// Stateful sliding window compressor.
/// Create once, compress many batches through it.
/// Dictionary accumulates domain knowledge across batches.
#[pyclass]
pub struct GlasikSliding {
    inner: crate::tokenizer::sliding::SlidingTokenizer,
}

#[pymethods]
impl GlasikSliding {
    #[new]
    fn new() -> Self {
        GlasikSliding {
            inner: crate::tokenizer::sliding::SlidingTokenizer::new(),
        }
    }

    fn compress(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use crate::codec::frame::{self, Frame};
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;

        let tokenized = self.inner.encode(data);

        // Auto-select: deflate or codon-only
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&tokenized).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        let deflated = enc.finish().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let framed = if deflated.len() < tokenized.len() {
            frame::encode(&Frame::new(deflated, true))
        } else {
            let mut f = Frame::new(tokenized, false);
            f.flags = crate::pipeline::FLAG_CODON_ONLY;
            frame::encode(&f)
        };

        Ok(PyBytes::new(py, &framed).into())
    }

    fn decompress(&self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        match crate::pipeline::decompress(data) {
            Ok(d) => Ok(PyBytes::new(py, &d).into()),
            Err(e) => Err(pyo3::exceptions::PyValueError::new_err(e.to_string())),
        }
    }

    fn stats(&self) -> (usize, u64) {
        self.inner.stats()
    }
}

#[pyfunction]
fn gn_ans_compress(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    let compressed = crate::codec::ans::compress(data);
    Ok(PyBytes::new(py, &compressed).into())
}

#[pyfunction]
fn gn_ans_decompress(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    match crate::codec::ans::decompress(data) {
        Some(d) => Ok(PyBytes::new(py, &d).into()),
        None => Err(pyo3::exceptions::PyValueError::new_err(
            "ANS decompress failed",
        )),
    }
}


#[pyfunction]
fn gn_ans_compress_bits(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    let compressed = crate::codec::ans::compress_bits(data);
    Ok(PyBytes::new(py, &compressed).into())
}

#[pyfunction]
fn gn_ans_decompress_bits(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    match crate::codec::ans::decompress_bits(data) {
        Some(d) => Ok(PyBytes::new(py, &d).into()),
        None => Err(pyo3::exceptions::PyValueError::new_err(
            "ANS bit decompress failed",
        )),
    }
}


#[pyfunction]
fn gn_ans_compress_o1(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    let compressed = crate::codec::ans::compress_o1(data);
    Ok(PyBytes::new(py, &compressed).into())
}

#[pyfunction]
fn gn_ans_decompress_o1(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    match crate::codec::ans::decompress_o1(data) {
        Some(d) => Ok(PyBytes::new(py, &d).into()),
        None => Err(pyo3::exceptions::PyValueError::new_err(
            "ANS O1 decompress failed",
        )),
    }
}


/// Stateful sliding window compressor v2 -- external dictionary, no per-frame overhead.
#[pyclass]
pub struct GlasikSlidingV2 {
    inner: crate::tokenizer::sliding_v2::SlidingTokenizerV2,
}

#[pymethods]
impl GlasikSlidingV2 {
    #[new]
    fn new() -> Self {
        GlasikSlidingV2 {
            inner: crate::tokenizer::sliding_v2::SlidingTokenizerV2::new(),
        }
    }

    fn compress(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;

        let tokenized = self.inner.encode(data);

        // Deflate the tokenized output (dict not in frame, so deflate sees clean data)
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&tokenized).unwrap();
        let deflated = enc.finish().unwrap();

        // Use smaller of tokenized vs deflated
        let out = if deflated.len() < tokenized.len() { deflated } else { tokenized };
        Ok(PyBytes::new(py, &out).into())
    }

    fn stats(&self) -> (usize, u64) {
        self.inner.stats()
    }

    fn dict_version(&self) -> u32 {
        self.inner.dict_version()
    }

    fn export_dict_json(&self) -> String {
        let (version, entries) = self.inner.export_dict();
        let mut parts: Vec<String> = Vec::new();
        for (bytes, freq, saving) in &entries {
            let bytes_str: Vec<String> = bytes.iter().map(|b| b.to_string()).collect();
            let mut entry = String::from("{");
            entry.push_str("\"b\":[");
            entry.push_str(&bytes_str.join(","));
            entry.push_str("],\"f\":");
            entry.push_str(&freq.to_string());
            entry.push_str(",\"s\":");
            entry.push_str(&saving.to_string());
            entry.push('}');
            parts.push(entry);
        }
        let mut out = String::from("{\"version\":");
        out.push_str(&version.to_string());
        out.push_str(",\"entries\":[");
        out.push_str(&parts.join(","));
        out.push_str("]}");
        out
    }

    fn import_dict(&mut self, version: u32, entries: Vec<(Vec<u8>, u64, u64)>) {
        self.inner.import_dict(version, entries);
    }

    #[staticmethod]
    fn with_bundled_dict() -> GlasikSlidingV2 {
        let entries = crate::static_dict::load_static_dict();
        GlasikSlidingV2 {
            inner: crate::tokenizer::sliding_v2::SlidingTokenizerV2::new_with_static(entries),
        }
    }

    #[staticmethod]
    fn from_static_dict(entries: Vec<(Vec<u8>, u64, u64)>) -> GlasikSlidingV2 {
        GlasikSlidingV2 {
            inner: crate::tokenizer::sliding_v2::SlidingTokenizerV2::new_with_static(entries),
        }
    }
}

/// Level 4 sliding window with fractal dictionary compression
#[pyclass]
pub struct GlasikSlidingL4 {
    inner: crate::sliding_v2_l4::SlidingTokenizerL4,
}

#[pymethods]
impl GlasikSlidingL4 {
    #[new]
    fn new() -> Self {
        GlasikSlidingL4 { inner: crate::sliding_v2_l4::SlidingTokenizerL4::new() }
    }

    #[staticmethod]
    fn with_bundled_dict() -> GlasikSlidingL4 {
        let entries = crate::static_dict::load_static_dict();
        GlasikSlidingL4 {
            inner: crate::sliding_v2_l4::SlidingTokenizerL4::new_with_static(entries),
        }
    }

    fn compress(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;
        use pyo3::exceptions::PyRuntimeError;

        let tokenized = self.inner.encode(data);
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&tokenized).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let deflated = enc.finish().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let out = if deflated.len() < tokenized.len() { deflated } else { tokenized };
        Ok(PyBytes::new(py, &out).into())
    }

    fn stats(&self) -> (usize, u64) { self.inner.stats() }

    fn snapshot_size(&self) -> usize { self.inner.snapshot_size() }

    fn get_snapshot(&self, py: Python) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self.inner.get_snapshot().map(|s| PyBytes::new(py, s).into()))
    }

    #[staticmethod]
    fn from_snapshot(py: Python, snapshot: &[u8]) -> GlasikSlidingL4 {
        GlasikSlidingL4 {
            inner: crate::sliding_v2_l4::SlidingTokenizerL4::restore_from_snapshot(snapshot),
        }
    }
}
#[pymodule]
fn glasik_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(gn_compress, m)?)?;
    m.add_function(wrap_pyfunction!(gn_decompress, m)?)?;
    m.add_function(wrap_pyfunction!(gn_compress_stats, m)?)?;
    m.add_function(wrap_pyfunction!(gn_compress_batch, m)?)?;
    m.add_function(wrap_pyfunction!(gn_compress_batch_stats, m)?)?;
    m.add_class::<GlasikSliding>()?;
    m.add_function(wrap_pyfunction!(gn_ans_compress, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_decompress, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_compress_bits, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_decompress_bits, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_compress_o1, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_decompress_o1, m)?)?;
    m.add_class::<GlasikSlidingV2>()?;
    m.add_class::<GlasikSlidingL4>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
