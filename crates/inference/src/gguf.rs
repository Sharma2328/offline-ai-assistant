//! GGUF file validation + metadata extraction (FR-MOD-001, spec §5.6 `inspectModel`).
//!
//! Reads only the GGUF header + key/value metadata block (no tensor data) to identify a
//! model, then streams the whole file to compute a SHA-256 checksum. Pure, offline, and
//! defensive: a non-GGUF, truncated, or malformed file is rejected with a typed error so
//! the command layer can surface `MODEL_INVALID` (never a panic).
//!
//! Reference: <https://github.com/ggerganov/ggml/blob/master/docs/gguf.md>

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;

/// GGUF magic bytes ("GGUF") at offset 0.
const MAGIC: [u8; 4] = *b"GGUF";
/// Largest metadata string we will read (guards against corrupt length prefixes).
const MAX_STRING_BYTES: u64 = 128 * 1024 * 1024;
/// Largest number of metadata KV pairs we will iterate (corruption guard).
const MAX_KV_COUNT: u64 = 1_000_000;
/// Read buffer for the checksum stream.
const HASH_CHUNK: usize = 1024 * 1024;

/// Model container format. Only GGUF is supported in the MVP (P1: extra formats).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat {
    Gguf,
}

/// Parsed model metadata (contract §8.1). Fields that a file does not declare are `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetadata {
    pub format: ModelFormat,
    pub architecture: Option<String>,
    pub parameter_count: Option<u64>,
    pub quantization: Option<String>,
    pub context_length_max: Option<u32>,
    pub size_bytes: u64,
    pub sha256: String,
    /// Preserved scalar metadata (stringified) for diagnostics; large arrays are elided.
    pub raw: BTreeMap<String, String>,
}

/// Reasons a file is not a usable GGUF model.
#[derive(Debug, Error)]
pub enum GgufError {
    /// The file does not start with the GGUF magic bytes.
    #[error("not a GGUF file (bad magic)")]
    BadMagic,
    /// The GGUF version is not one this build understands.
    #[error("unsupported GGUF version {0}")]
    UnsupportedVersion(u32),
    /// The header/metadata block is structurally invalid.
    #[error("malformed GGUF: {0}")]
    Malformed(String),
    /// An I/O error (missing file, permissions, truncation).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Validate a GGUF file and extract its metadata + SHA-256 checksum (FR-MOD-001).
pub fn inspect_model(path: &Path) -> Result<ModelMetadata, GgufError> {
    let size_bytes = std::fs::metadata(path)?.len();

    let file = File::open(path)?;
    let mut reader = GgufReader::new(BufReader::new(file));
    let (architecture, parameter_count, quantization, context_length_max, raw) =
        reader.parse_header()?;

    let sha256 = compute_sha256(path)?;

    Ok(ModelMetadata {
        format: ModelFormat::Gguf,
        architecture,
        parameter_count,
        quantization,
        context_length_max,
        size_bytes,
        sha256,
        raw,
    })
}

/// Stream a file and return its lowercase hex SHA-256 digest.
pub fn compute_sha256(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; HASH_CHUNK];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    Ok(out)
}

/// Map a GGML `general.file_type` enum value to a human quantization label. Unknown values
/// fall back to `ftype:<n>` so the raw signal is never lost.
fn quantization_label(file_type: u32) -> String {
    let label = match file_type {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        7 => "Q8_0",
        8 => "Q5_0",
        9 => "Q5_1",
        10 => "Q2_K",
        11 => "Q3_K_S",
        12 => "Q3_K_M",
        13 => "Q3_K_L",
        14 => "Q4_K_S",
        15 => "Q4_K_M",
        16 => "Q5_K_S",
        17 => "Q5_K_M",
        18 => "Q6_K",
        19 => "Q8_K",
        20 => "IQ2_XXS",
        21 => "IQ2_XS",
        22 => "IQ3_XXS",
        23 => "IQ1_S",
        24 => "IQ4_NL",
        25 => "IQ3_S",
        26 => "IQ2_S",
        27 => "IQ4_XS",
        28 => "IQ1_M",
        _ => return format!("ftype:{file_type}"),
    };
    label.to_string()
}

/// A little-endian reader over the GGUF header + metadata block.
struct GgufReader<R: Read + Seek> {
    inner: R,
}

impl<R: Read + Seek> GgufReader<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }

    /// Parse magic, version, and the metadata KV block, returning the fields we surface.
    #[allow(clippy::type_complexity)]
    fn parse_header(
        &mut self,
    ) -> Result<
        (
            Option<String>,
            Option<u64>,
            Option<String>,
            Option<u32>,
            BTreeMap<String, String>,
        ),
        GgufError,
    > {
        let mut magic = [0u8; 4];
        self.inner.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(GgufError::BadMagic);
        }

        let version = self.read_u32()?;
        // v1 used 32-bit counts/lengths; the MVP targets the modern 64-bit layout (v2/v3).
        if !(2..=3).contains(&version) {
            return Err(GgufError::UnsupportedVersion(version));
        }

        let _tensor_count = self.read_u64()?;
        let kv_count = self.read_u64()?;
        if kv_count > MAX_KV_COUNT {
            return Err(GgufError::Malformed(format!(
                "metadata KV count {kv_count} exceeds the sane maximum"
            )));
        }

        let mut raw: BTreeMap<String, String> = BTreeMap::new();
        for _ in 0..kv_count {
            let key = self.read_string()?;
            let value_type = self.read_u32()?;
            if value_type == 9 {
                // Array: record a compact placeholder and skip the elements.
                let elem_type = self.read_u32()?;
                let count = self.read_u64()?;
                self.skip_array(elem_type, count)?;
                raw.insert(key, format!("[array; {count} items]"));
            } else if let Some(scalar) = self.read_scalar(value_type)? {
                raw.insert(key, scalar);
            }
        }

        let architecture = raw.get("general.architecture").cloned();

        let quantization = raw
            .get("general.file_type")
            .and_then(|v| v.parse::<u32>().ok())
            .map(quantization_label);

        let parameter_count = raw
            .get("general.parameter_count")
            .and_then(|v| v.parse::<u64>().ok());

        let context_length_max = architecture
            .as_deref()
            .and_then(|arch| raw.get(&format!("{arch}.context_length")))
            .and_then(|v| v.parse::<u64>().ok())
            .and_then(|v| u32::try_from(v).ok());

        Ok((
            architecture,
            parameter_count,
            quantization,
            context_length_max,
            raw,
        ))
    }

    /// Read a scalar metadata value, returning its stringified form. `None` means the type
    /// carries no useful scalar (arrays are handled by the caller).
    fn read_scalar(&mut self, value_type: u32) -> Result<Option<String>, GgufError> {
        let repr = match value_type {
            0 => self.read_u8()?.to_string(),
            1 => (self.read_u8()? as i8).to_string(),
            2 => self.read_u16()?.to_string(),
            3 => (self.read_u16()? as i16).to_string(),
            4 => self.read_u32()?.to_string(),
            5 => (self.read_u32()? as i32).to_string(),
            6 => f32::from_bits(self.read_u32()?).to_string(),
            7 => (self.read_u8()? != 0).to_string(),
            8 => self.read_string()?,
            10 => self.read_u64()?.to_string(),
            11 => (self.read_u64()? as i64).to_string(),
            12 => f64::from_bits(self.read_u64()?).to_string(),
            other => {
                return Err(GgufError::Malformed(format!(
                    "unknown metadata value type {other}"
                )))
            }
        };
        Ok(Some(repr))
    }

    /// Skip `count` elements of the given array element type without materializing them.
    fn skip_array(&mut self, elem_type: u32, count: u64) -> Result<(), GgufError> {
        let fixed = match elem_type {
            0 | 1 | 7 => 1u64,
            2 | 3 => 2,
            4..=6 => 4,
            10..=12 => 8,
            8 => {
                // Array of strings: each is a length-prefixed blob.
                for _ in 0..count {
                    let len = self.read_u64()?;
                    self.skip_bytes(len)?;
                }
                return Ok(());
            }
            9 => {
                // Nested arrays: recurse element by element.
                for _ in 0..count {
                    let inner_type = self.read_u32()?;
                    let inner_count = self.read_u64()?;
                    self.skip_array(inner_type, inner_count)?;
                }
                return Ok(());
            }
            other => {
                return Err(GgufError::Malformed(format!(
                    "unknown array element type {other}"
                )))
            }
        };
        let total = fixed.checked_mul(count).ok_or_else(|| {
            GgufError::Malformed("array size overflows a 64-bit offset".to_string())
        })?;
        self.skip_bytes(total)
    }

    fn skip_bytes(&mut self, n: u64) -> Result<(), GgufError> {
        let offset = i64::try_from(n)
            .map_err(|_| GgufError::Malformed("skip offset overflows i64".to_string()))?;
        self.inner.seek(SeekFrom::Current(offset))?;
        Ok(())
    }

    fn read_string(&mut self) -> Result<String, GgufError> {
        let len = self.read_u64()?;
        if len > MAX_STRING_BYTES {
            return Err(GgufError::Malformed(format!(
                "metadata string length {len} exceeds the sane maximum"
            )));
        }
        let mut buf = vec![0u8; usize::try_from(len).unwrap_or(usize::MAX)];
        self.inner.read_exact(&mut buf)?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    fn read_u8(&mut self) -> Result<u8, GgufError> {
        let mut b = [0u8; 1];
        self.inner.read_exact(&mut b)?;
        Ok(b[0])
    }

    fn read_u16(&mut self) -> Result<u16, GgufError> {
        let mut b = [0u8; 2];
        self.inner.read_exact(&mut b)?;
        Ok(u16::from_le_bytes(b))
    }

    fn read_u32(&mut self) -> Result<u32, GgufError> {
        let mut b = [0u8; 4];
        self.inner.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    fn read_u64(&mut self) -> Result<u64, GgufError> {
        let mut b = [0u8; 8];
        self.inner.read_exact(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a minimal but valid GGUF v3 file with the given string/u32 metadata pairs.
    fn write_gguf(
        path: &Path,
        strings: &[(&str, &str)],
        u32s: &[(&str, u32)],
        arrays: &[(&str, u32, u64)],
    ) {
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&3u32.to_le_bytes()); // version
        buf.extend_from_slice(&0u64.to_le_bytes()); // tensor_count
        let kv = (strings.len() + u32s.len() + arrays.len()) as u64;
        buf.extend_from_slice(&kv.to_le_bytes());

        let put_str = |buf: &mut Vec<u8>, s: &str| {
            buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
            buf.extend_from_slice(s.as_bytes());
        };

        for (k, v) in strings {
            put_str(&mut buf, k);
            buf.extend_from_slice(&8u32.to_le_bytes()); // STRING
            put_str(&mut buf, v);
        }
        for (k, v) in u32s {
            put_str(&mut buf, k);
            buf.extend_from_slice(&4u32.to_le_bytes()); // UINT32
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for (k, elem_type, count) in arrays {
            put_str(&mut buf, k);
            buf.extend_from_slice(&9u32.to_le_bytes()); // ARRAY
            buf.extend_from_slice(&elem_type.to_le_bytes());
            buf.extend_from_slice(&count.to_le_bytes());
            // Only u32 element arrays are emitted by tests.
            for i in 0..*count {
                buf.extend_from_slice(&(i as u32).to_le_bytes());
            }
        }

        let mut f = File::create(path).unwrap();
        f.write_all(&buf).unwrap();
    }

    #[test]
    fn parses_architecture_context_and_quantization() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.gguf");
        write_gguf(
            &path,
            &[
                ("general.architecture", "llama"),
                ("general.name", "Test 7B"),
            ],
            &[("general.file_type", 15), ("llama.context_length", 8192)],
            &[],
        );

        let meta = inspect_model(&path).unwrap();
        assert_eq!(meta.format, ModelFormat::Gguf);
        assert_eq!(meta.architecture.as_deref(), Some("llama"));
        assert_eq!(meta.context_length_max, Some(8192));
        assert_eq!(meta.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(
            meta.raw.get("general.name").map(String::as_str),
            Some("Test 7B")
        );
        assert_eq!(meta.sha256.len(), 64);
        assert!(meta.size_bytes > 0);
    }

    #[test]
    fn skips_array_metadata_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.gguf");
        write_gguf(
            &path,
            &[("general.architecture", "llama")],
            &[("llama.context_length", 4096)],
            &[("llama.some_array", 4, 5)], // 5 x u32
        );

        let meta = inspect_model(&path).unwrap();
        assert_eq!(meta.context_length_max, Some(4096));
        assert!(meta
            .raw
            .get("llama.some_array")
            .is_some_and(|v| v.contains("array")));
    }

    #[test]
    fn rejects_non_gguf_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-model.bin");
        std::fs::write(&path, b"this is definitely not a gguf file").unwrap();

        assert!(matches!(inspect_model(&path), Err(GgufError::BadMagic)));
    }

    #[test]
    fn rejects_truncated_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("truncated.gguf");
        // Valid magic but nothing after it.
        std::fs::write(&path, MAGIC).unwrap();

        assert!(matches!(inspect_model(&path), Err(GgufError::Io(_))));
    }

    #[test]
    fn rejects_unsupported_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.gguf");
        let mut buf = Vec::new();
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&1u32.to_le_bytes());
        std::fs::write(&path, buf).unwrap();

        assert!(matches!(
            inspect_model(&path),
            Err(GgufError::UnsupportedVersion(1))
        ));
    }

    #[test]
    fn sha256_is_stable_for_identical_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.bin");
        let b = dir.path().join("b.bin");
        std::fs::write(&a, b"identical contents").unwrap();
        std::fs::write(&b, b"identical contents").unwrap();
        assert_eq!(compute_sha256(&a).unwrap(), compute_sha256(&b).unwrap());
    }

    #[test]
    fn quantization_label_falls_back_for_unknown_ftype() {
        assert_eq!(quantization_label(15), "Q4_K_M");
        assert_eq!(quantization_label(999), "ftype:999");
    }
}
