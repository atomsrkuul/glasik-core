use crate::pipeline::{compress, compress_batch_with_stats, compress_with_stats, decompress};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

#[pyfunction]
fn gn_compress(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    Ok(PyBytes::new(py, &compress(data)).into())
}

#[pyfunction]
fn gn_compress_o1(py: Python, data: &[u8], freq_table: &[u8]) -> PyResult<Py<PyBytes>> {
    // Full GNC pipeline: tokenize -> o1 ANS pretrained (no deflate)
    let tok = crate::tokenizer::Tokenizer::new();
    let (tokenized, _) = tok.encode(data);
    match crate::codec::ans::compress_o1_pretrained(&tokenized, freq_table) {
        Some(c) => Ok(PyBytes::new(py, &c).into()),
        None => Err(pyo3::exceptions::PyValueError::new_err("o1 compress failed")),
    }
}

#[pyfunction]
fn gn_decompress_o1(py: Python, data: &[u8], freq_table: &[u8]) -> PyResult<Py<PyBytes>> {
    let tok = crate::tokenizer::Tokenizer::new();
    match crate::codec::ans::decompress_o1_pretrained(data, freq_table) {
        Some(tokenized) => {
            tok.decode(&tokenized)
                .map(|d| PyBytes::new(py, &d).into())
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
        }
        None => Err(pyo3::exceptions::PyValueError::new_err("o1 decompress failed")),
    }
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

        let tokenized = self.inner.encode_ac(data);

        // Auto-select: deflate or codon-only
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
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


#[pyfunction]
fn gn_ans_train(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    let freq = crate::codec::ans::FreqTable::build(data);
    Ok(PyBytes::new(py, &freq.serialize()).into())
}

#[pyfunction]
fn gn_ans_compress_pretrained(py: Python, data: &[u8], freq_bytes: &[u8]) -> PyResult<Py<PyBytes>> {
    match crate::codec::ans::FreqTable::deserialize(freq_bytes) {
        Some((freq, _)) => {
            let compressed = crate::codec::ans::compress_with_table(data, &freq);
            Ok(PyBytes::new(py, &compressed).into())
        }
        None => Err(pyo3::exceptions::PyValueError::new_err("invalid freq table")),
    }
}

#[pyfunction]
fn gn_ans_decompress_pretrained(py: Python, data: &[u8], freq_bytes: &[u8]) -> PyResult<Py<PyBytes>> {
    match crate::codec::ans::FreqTable::deserialize(freq_bytes) {
        Some((freq, _)) => {
            match crate::codec::ans::decompress_with_table(data, &freq) {
                Some(d) => Ok(PyBytes::new(py, &d).into()),
                None => Err(pyo3::exceptions::PyValueError::new_err("ANS decompress failed")),
            }
        }
        None => Err(pyo3::exceptions::PyValueError::new_err("invalid freq table")),
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
    fn ingest_fast(&mut self, data: &[u8]) {
        self.inner.ingest_fast(data);
    }

    fn decode_ac_split(&mut self, py: Python, original: &[u8], tok_ids: &[u8], literals: &[u8]) -> PyResult<Py<PyBytes>> {
        let out = self.inner.decode_ac_split(original, tok_ids, literals);
        Ok(PyBytes::new(py, &out).into())
    }

    fn encode_ac_split(&mut self, py: Python, data: &[u8]) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
        let (toks, lits) = self.inner.encode_ac_split(data);
        Ok((PyBytes::new(py, &toks).into(), PyBytes::new(py, &lits).into()))
    }

    fn encode_ac_raw(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        let tokenized = self.inner.encode_ac(data);
        Ok(PyBytes::new(py, &tokenized).into())
    }

    fn compress_ac_cached(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;
        use pyo3::exceptions::PyRuntimeError;
        let tokenized = self.inner.encode_ac(data);
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&tokenized).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let deflated = enc.finish().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let out = if deflated.len() < tokenized.len() { deflated } else { tokenized };
        Ok(PyBytes::new(py, &out).into())
    }

    fn compress_ac_greedy(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;
        use pyo3::exceptions::PyRuntimeError;
        let tokenized = self.inner.encode_ac_greedy(data);
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&tokenized).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let deflated = enc.finish().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let out = if deflated.len() < tokenized.len() { deflated } else { tokenized };
        Ok(PyBytes::new(py, &out).into())
    }

    fn compress_ac(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;
        use pyo3::exceptions::PyRuntimeError;
        let active = self.inner.active_entries_pub();
        let tokenized = crate::tokenizer::codon::encode_ac(data, &active);
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&tokenized).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let deflated = enc.finish().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let out = if deflated.len() < tokenized.len() { deflated } else { tokenized };
        Ok(PyBytes::new(py, &out).into())
    }

    fn encode_raw(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        // Returns raw tokenized bytes BEFORE deflate -- for analysis only
        let tokenized = self.inner.encode(data);
        Ok(PyBytes::new(py, &tokenized).into())
    }


    fn compress_ans(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        let tokenized = self.inner.encode(data);
        let compressed = crate::codec::ans::compress(&tokenized);
        let mut out = if compressed.len() < tokenized.len() {
            let mut v = vec![0x01u8];
            v.extend_from_slice(&compressed);
            v
        } else {
            let mut v = vec![0x00u8];
            v.extend_from_slice(&tokenized);
            v
        };
        Ok(PyBytes::new(py, &out).into())
    }

    fn compress_backref(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        let tokenized = self.inner.encode_ac(data);
        let backreffed = crate::codec::backref::compress(&tokenized);
        let out = if backreffed.len() < tokenized.len() { backreffed } else { tokenized };
        Ok(PyBytes::new(py, &out).into())
    }


    fn compress_v4(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        let tokenized = self.inner.encode_ac(data);
        let backreffed = crate::codec::backref::compress(&tokenized);
        let ans_out = crate::codec::ans::compress(&backreffed);
        let out = if ans_out.len() < backreffed.len() {
            let mut v = vec![0x01u8];
            v.extend_from_slice(&ans_out);
            v
        } else {
            let mut v = vec![0x00u8];
            v.extend_from_slice(&backreffed);
            v
        };
        Ok(PyBytes::new(py, &out).into())
    }

    fn decompress_v4(&self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use pyo3::exceptions::PyRuntimeError;
        if data.is_empty() { return Ok(PyBytes::new(py, &[]).into()); }
        let flag = data[0];
        let payload = &data[1..];
        let backreffed = if flag == 0x01 {
            match crate::codec::ans::decompress(payload) {
                Some(d) => d,
                None => return Err(PyRuntimeError::new_err("v4 ANS decompress failed")),
            }
        } else {
            payload.to_vec()
        };
        let tokenized = crate::codec::backref::decompress(&backreffed);
        match self.inner.decode_raw(&tokenized) {
            Ok(original) => Ok(PyBytes::new(py, &original).into()),
            Err(e) => Err(PyRuntimeError::new_err(format!("v4 decode failed: {}", e))),
        }
    }
    fn decompress_backref(&self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use pyo3::exceptions::PyRuntimeError;
        let debackreffed = crate::codec::backref::decompress(data);
        match self.inner.decode_raw(&debackreffed) {
            Ok(original) => Ok(PyBytes::new(py, &original).into()),
            Err(e) => Err(PyRuntimeError::new_err(format!("backref decode failed: {}", e))),
        }
    }

    fn decompress_ans(&self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use pyo3::exceptions::PyRuntimeError;
        if data.is_empty() {
            return Ok(PyBytes::new(py, &[]).into());
        }
        let flag = data[0];
        let payload = &data[1..];
        let tokenized = if flag == 0x01 {
            match crate::codec::ans::decompress(payload) {
                Some(t) => t,
                None => return Err(PyRuntimeError::new_err("ANS decompress failed")),
            }
        } else {
            payload.to_vec()
        };
        match self.inner.decode(&tokenized) {
            Ok(original) => Ok(PyBytes::new(py, &original).into()),
            Err(e) => Err(PyRuntimeError::new_err(format!("decode failed: {}", e))),
        }
    }


    fn compress(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;

        let tokenized = self.inner.encode_ac(data);

        // Deflate the tokenized output (dict not in frame, so deflate sees clean data)
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        let _ = enc.write_all(&tokenized);
        let deflated = enc.finish().unwrap_or_default();

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
pub struct GlasikSlidingV3 {
    inner: crate::sliding_v3::SlidingTokenizerV3,
}

#[pymethods]
impl GlasikSlidingV3 {
    #[new]
    fn new() -> Self {
        GlasikSlidingV3 { inner: crate::sliding_v3::SlidingTokenizerV3::new() }
    }

    #[staticmethod]
    fn with_bundled_dict() -> GlasikSlidingV3 {
        let entries = crate::static_dict::load_static_dict();
        GlasikSlidingV3 {
            inner: crate::sliding_v3::SlidingTokenizerV3::new_with_static(entries),
        }
    }

    fn ingest_fast(&mut self, data: &[u8]) {
        self.inner.ingest_fast(data);
    }

    fn compress(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;
        use pyo3::exceptions::PyRuntimeError;

        let tokenized = self.inner.encode(data);
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
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
    fn from_snapshot(py: Python, snapshot: &[u8]) -> GlasikSlidingV3 {
        GlasikSlidingV3 {
            inner: crate::sliding_v3::SlidingTokenizerV3::restore_from_snapshot(snapshot),
        }
    }
}

#[pyfunction]
fn gn_compress_parallel(py: Python, chunks: Vec<Vec<u8>>) -> PyResult<Py<PyList>> {
    use rayon::prelude::*;
    let compressed: Vec<Vec<u8>> = chunks.par_iter()
        .map(|chunk| crate::pipeline::compress(chunk))
        .collect();
    let list = PyList::empty(py);
    for c in compressed {
        list.append(PyBytes::new(py, &c))?;
    }
    Ok(list.into())
}


#[pyclass]
pub struct GNHybridEncoder {
    inner: crate::tokenizer::lz77_gn::GNHybridEncoder<4>,
}

#[pymethods]
impl GNHybridEncoder {
    #[new]
    fn new() -> Self {
        GNHybridEncoder { inner: crate::tokenizer::lz77_gn::GNHybridEncoder::new() }
    }

    fn seed_vocab(&mut self, entries: Vec<(Vec<u8>, usize, usize)>) {
        let dict: Vec<crate::tokenizer::dictionary::DictEntry> = entries.into_iter()
            .map(|(bytes, freq, saving)| crate::tokenizer::dictionary::DictEntry { bytes, freq, saving })
            .collect();
        self.inner.seed_vocab(&dict);
    }

    fn encode(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;
        use pyo3::exceptions::PyRuntimeError;
        let tokenized = self.inner.encode(data);
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&tokenized).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let deflated = enc.finish().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let out = if deflated.len() < tokenized.len() { deflated } else { tokenized };
        Ok(PyBytes::new(py, &out).into())
    }
}

#[pyclass]
pub struct GNHybridAsync {
    inner: crate::tokenizer::hybrid_async::HybridAsyncEncoder,
}

#[pymethods]
impl GNHybridAsync {
    #[new]
    fn new() -> Self {
        GNHybridAsync { inner: crate::tokenizer::hybrid_async::HybridAsyncEncoder::new() }
    }

    fn encode(&mut self, py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
        let out = self.inner.encode(data);
        Ok(PyBytes::new(py, &out).into())
    }

    fn stats(&self) -> (usize, u64, u64) {
        self.inner.stats()
    }
}

#[pymodule]
fn glasik_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(gn_compress, m)?)?;
    m.add_function(wrap_pyfunction!(gn_compress_o1, m)?)?;
    m.add_function(wrap_pyfunction!(gn_decompress_o1, m)?)?;
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

#[pyfunction]
fn gn_ans_train_o1(py: Python, data: &[u8]) -> PyResult<Py<PyBytes>> {
    let tbl = crate::codec::ans::train_o1(data);
    Ok(PyBytes::new(py, &tbl).into())
}

#[pyfunction]
fn gn_ans_compress_pretrained_o1(py: Python, data: &[u8], tbl: &[u8]) -> PyResult<Py<PyBytes>> {
    match crate::codec::ans::compress_o1_pretrained(data, tbl) {
        Some(c) => Ok(PyBytes::new(py, &c).into()),
        None => Err(pyo3::exceptions::PyValueError::new_err("compress_o1_pretrained failed")),
    }
}

#[pyfunction]
fn gn_ans_decompress_pretrained_o1(py: Python, data: &[u8], tbl: &[u8]) -> PyResult<Py<PyBytes>> {
    match crate::codec::ans::decompress_o1_pretrained(data, tbl) {
        Some(d) => Ok(PyBytes::new(py, &d).into()),
        None => Err(pyo3::exceptions::PyValueError::new_err("decompress_o1_pretrained failed")),
    }
}
    m.add_function(wrap_pyfunction!(gn_ans_train, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_train_o1, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_compress_pretrained_o1, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_decompress_pretrained_o1, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_compress_pretrained, m)?)?;
    m.add_function(wrap_pyfunction!(gn_ans_decompress_pretrained, m)?);
    m.add_class::<GlasikSlidingV2>()?;
    m.add_class::<GlasikSlidingV3>()?;
    m.add_class::<GNHybridEncoder>()?;
    m.add_class::<GNHybridAsync>()?;
    m.add_function(wrap_pyfunction!(gn_compress_parallel, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
