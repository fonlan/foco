//! Append-only Zstd segment store for LLM wire audit dumps.
//!
//! SQLite keeps structured metrics, event indexes, and segment locators.
//! Raw HTTP request/response JSON lives in `.foco/llm-audit/segments/seg-*.focoaud`.

use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::private_fs::{create_private_dir_all, prepare_private_file, restrict_private_file};

/// Directory under `.foco` that holds segment files.
pub const LLM_AUDIT_SEGMENTS_DIR: &str = "llm-audit/segments";

/// Rotate to a new segment after this many bytes (compressed file size).
pub const LLM_AUDIT_SEGMENT_ROTATE_BYTES: u64 = 256 * 1024 * 1024;

const FILE_MAGIC: &[u8; 8] = b"FOCOAUD1";
const RECORD_MAGIC: &[u8; 4] = b"FDAT";
const FILE_HEADER_LEN: u64 = 16;
const RECORD_HEADER_LEN: usize = 4 + 1 + 1 + 2 + 4 + 4 + 32; // 48

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LlmAuditDetailKind {
    Request = 1,
    Response = 2,
}

impl LlmAuditDetailKind {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Request),
            2 => Some(Self::Response),
            _ => None,
        }
    }
}

/// Locator stored in SQLite for one compressed detail blob.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmAuditDetailLocator {
    pub segment_id: i64,
    pub offset: u64,
    pub compressed_len: u32,
    pub uncompressed_len: u32,
    pub sha256_hex: String,
}

#[derive(Debug)]
pub enum LlmAuditSegmentError {
    Io { path: PathBuf, source: io::Error },
    Corrupt { path: PathBuf, message: String },
    Sqlite { source: rusqlite::Error },
}

impl std::fmt::Display for LlmAuditSegmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Corrupt { path, message } => {
                write!(f, "corrupt LLM audit segment {}: {message}", path.display())
            }
            Self::Sqlite { source } => write!(f, "LLM audit segment metadata: {source}"),
        }
    }
}

impl std::error::Error for LlmAuditSegmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Sqlite { source } => Some(source),
            Self::Corrupt { .. } => None,
        }
    }
}

/// Owns the on-disk segment directory for one workspace database.
#[derive(Debug)]
pub struct LlmAuditSegmentStore {
    segments_dir: PathBuf,
}

impl LlmAuditSegmentStore {
    pub fn open(foco_dir: impl AsRef<Path>) -> Result<Self, LlmAuditSegmentError> {
        let segments_dir = foco_dir.as_ref().join(LLM_AUDIT_SEGMENTS_DIR);
        create_private_dir_all(&segments_dir).map_err(|source| LlmAuditSegmentError::Io {
            path: segments_dir.clone(),
            source,
        })?;
        Ok(Self { segments_dir })
    }

    /// Append UTF-8 JSON detail; returns a durable locator after `fsync`.
    pub fn append_detail(
        &self,
        connection: &Connection,
        kind: LlmAuditDetailKind,
        utf8_json: &str,
    ) -> Result<LlmAuditDetailLocator, LlmAuditSegmentError> {
        let uncompressed = utf8_json.as_bytes();
        let uncompressed_len =
            u32::try_from(uncompressed.len()).map_err(|_| LlmAuditSegmentError::Corrupt {
                path: self.segments_dir.clone(),
                message: format!("detail payload too large ({} bytes)", uncompressed.len()),
            })?;
        let mut hasher = Sha256::new();
        hasher.update(uncompressed);
        let digest = hasher.finalize();
        let sha256_hex = hex_encode(&digest);

        let compressed =
            zstd::encode_all(uncompressed, 3).map_err(|source| LlmAuditSegmentError::Io {
                path: self.segments_dir.clone(),
                source: io::Error::new(io::ErrorKind::Other, source),
            })?;
        let compressed_len =
            u32::try_from(compressed.len()).map_err(|_| LlmAuditSegmentError::Corrupt {
                path: self.segments_dir.clone(),
                message: format!("compressed detail too large ({} bytes)", compressed.len()),
            })?;

        let (segment_id, file_name, mut file, start_offset) =
            self.open_active_segment_for_append(connection)?;

        let mut record = Vec::with_capacity(RECORD_HEADER_LEN + compressed.len());
        record.extend_from_slice(RECORD_MAGIC);
        record.push(kind as u8);
        record.push(0); // flags
        record.extend_from_slice(&0u16.to_le_bytes()); // reserved
        record.extend_from_slice(&uncompressed_len.to_le_bytes());
        record.extend_from_slice(&compressed_len.to_le_bytes());
        record.extend_from_slice(&digest);
        record.extend_from_slice(&compressed);

        file.write_all(&record)
            .map_err(|source| LlmAuditSegmentError::Io {
                path: self.segment_path(&file_name),
                source,
            })?;
        file.sync_all().map_err(|source| LlmAuditSegmentError::Io {
            path: self.segment_path(&file_name),
            source,
        })?;

        let new_size = start_offset
            .checked_add(record.len() as u64)
            .ok_or_else(|| LlmAuditSegmentError::Corrupt {
                path: self.segment_path(&file_name),
                message: "segment size overflow".to_string(),
            })?;
        connection
            .execute(
                "UPDATE llm_audit_segments
                 SET byte_size = ?2,
                     record_count = record_count + 1
                 WHERE id = ?1",
                params![segment_id, new_size as i64],
            )
            .map_err(|source| LlmAuditSegmentError::Sqlite { source })?;

        if new_size >= LLM_AUDIT_SEGMENT_ROTATE_BYTES {
            let closed_at = chrono_now();
            connection
                .execute(
                    "UPDATE llm_audit_segments SET closed_at = ?2 WHERE id = ?1 AND closed_at IS NULL",
                    params![segment_id, closed_at],
                )
                .map_err(|source| LlmAuditSegmentError::Sqlite { source })?;
        }

        Ok(LlmAuditDetailLocator {
            segment_id,
            offset: start_offset,
            compressed_len,
            uncompressed_len,
            sha256_hex,
        })
    }

    pub fn read_detail(
        &self,
        connection: &Connection,
        locator: &LlmAuditDetailLocator,
    ) -> Result<String, LlmAuditSegmentError> {
        let file_name: String = connection
            .query_row(
                "SELECT file_name FROM llm_audit_segments WHERE id = ?1",
                params![locator.segment_id],
                |row| row.get(0),
            )
            .map_err(|source| LlmAuditSegmentError::Sqlite { source })?;
        let path = self.segment_path(&file_name);
        let mut file = File::open(&path).map_err(|source| LlmAuditSegmentError::Io {
            path: path.clone(),
            source,
        })?;
        file.seek(SeekFrom::Start(locator.offset))
            .map_err(|source| LlmAuditSegmentError::Io {
                path: path.clone(),
                source,
            })?;

        let mut header = [0u8; RECORD_HEADER_LEN];
        file.read_exact(&mut header)
            .map_err(|source| LlmAuditSegmentError::Io {
                path: path.clone(),
                source,
            })?;
        if &header[0..4] != RECORD_MAGIC {
            return Err(LlmAuditSegmentError::Corrupt {
                path: path.clone(),
                message: "invalid record magic".to_string(),
            });
        }
        let _kind = LlmAuditDetailKind::from_u8(header[4]).ok_or_else(|| {
            LlmAuditSegmentError::Corrupt {
                path: path.clone(),
                message: format!("unknown detail kind {}", header[4]),
            }
        })?;
        let uncompressed_len = u32::from_le_bytes(header[8..12].try_into().unwrap());
        let compressed_len = u32::from_le_bytes(header[12..16].try_into().unwrap());
        let digest = &header[16..48];
        if uncompressed_len != locator.uncompressed_len || compressed_len != locator.compressed_len
        {
            return Err(LlmAuditSegmentError::Corrupt {
                path: path.clone(),
                message: "locator length mismatch".to_string(),
            });
        }

        let mut compressed = vec![0u8; compressed_len as usize];
        file.read_exact(&mut compressed)
            .map_err(|source| LlmAuditSegmentError::Io {
                path: path.clone(),
                source,
            })?;
        let plain =
            zstd::decode_all(compressed.as_slice()).map_err(|source| LlmAuditSegmentError::Io {
                path: path.clone(),
                source: io::Error::new(io::ErrorKind::InvalidData, source),
            })?;
        if plain.len() != uncompressed_len as usize {
            return Err(LlmAuditSegmentError::Corrupt {
                path: path.clone(),
                message: "decompressed length mismatch".to_string(),
            });
        }
        let mut hasher = Sha256::new();
        hasher.update(&plain);
        let actual = hasher.finalize();
        if actual.as_slice() != digest {
            return Err(LlmAuditSegmentError::Corrupt {
                path: path.clone(),
                message: "sha256 mismatch".to_string(),
            });
        }
        let expected = hex_encode(&actual);
        if expected != locator.sha256_hex {
            return Err(LlmAuditSegmentError::Corrupt {
                path: path.clone(),
                message: "locator sha256 mismatch".to_string(),
            });
        }
        String::from_utf8(plain).map_err(|source| LlmAuditSegmentError::Corrupt {
            path,
            message: format!("detail is not utf-8: {source}"),
        })
    }

    fn open_active_segment_for_append(
        &self,
        connection: &Connection,
    ) -> Result<(i64, String, File, u64), LlmAuditSegmentError> {
        let active: Option<(i64, String, i64)> = connection
            .query_row(
                "SELECT id, file_name, byte_size
                 FROM llm_audit_segments
                 WHERE closed_at IS NULL
                 ORDER BY id DESC
                 LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|source| LlmAuditSegmentError::Sqlite { source })?;

        if let Some((id, file_name, byte_size)) = active {
            if (byte_size as u64) < LLM_AUDIT_SEGMENT_ROTATE_BYTES {
                let path = self.segment_path(&file_name);
                let mut file = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(&path)
                    .map_err(|source| LlmAuditSegmentError::Io {
                        path: path.clone(),
                        source,
                    })?;
                let len =
                    file.seek(SeekFrom::End(0))
                        .map_err(|source| LlmAuditSegmentError::Io {
                            path: path.clone(),
                            source,
                        })?;
                if len < FILE_HEADER_LEN {
                    return Err(LlmAuditSegmentError::Corrupt {
                        path,
                        message: "segment shorter than header".to_string(),
                    });
                }
                // Prefer on-disk length if metadata lagged after a crash.
                let offset = len.max(byte_size as u64);
                if offset != byte_size as u64 {
                    connection
                        .execute(
                            "UPDATE llm_audit_segments SET byte_size = ?2 WHERE id = ?1",
                            params![id, offset as i64],
                        )
                        .map_err(|source| LlmAuditSegmentError::Sqlite { source })?;
                }
                return Ok((id, file_name, file, offset));
            }
            let closed_at = chrono_now();
            connection
                .execute(
                    "UPDATE llm_audit_segments SET closed_at = ?2 WHERE id = ?1 AND closed_at IS NULL",
                    params![id, closed_at],
                )
                .map_err(|source| LlmAuditSegmentError::Sqlite { source })?;
        }

        self.create_segment(connection)
    }

    fn create_segment(
        &self,
        connection: &Connection,
    ) -> Result<(i64, String, File, u64), LlmAuditSegmentError> {
        let created_at = chrono_now();
        connection
            .execute(
                "INSERT INTO llm_audit_segments (file_name, created_at, closed_at, byte_size, record_count)
                 VALUES ('pending', ?1, NULL, 0, 0)",
                params![created_at],
            )
            .map_err(|source| LlmAuditSegmentError::Sqlite { source })?;
        let id = connection.last_insert_rowid();
        let file_name = format!("seg-{id:08}.focoaud");
        connection
            .execute(
                "UPDATE llm_audit_segments SET file_name = ?2, byte_size = ?3 WHERE id = ?1",
                params![id, file_name, FILE_HEADER_LEN as i64],
            )
            .map_err(|source| LlmAuditSegmentError::Sqlite { source })?;

        let path = self.segment_path(&file_name);
        prepare_private_file(&path).map_err(|source| LlmAuditSegmentError::Io {
            path: path.clone(),
            source,
        })?;
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|source| LlmAuditSegmentError::Io {
                path: path.clone(),
                source,
            })?;
        let mut header = [0u8; FILE_HEADER_LEN as usize];
        header[0..8].copy_from_slice(FILE_MAGIC);
        header[8..10].copy_from_slice(&1u16.to_le_bytes()); // format version
        file.write_all(&header)
            .map_err(|source| LlmAuditSegmentError::Io {
                path: path.clone(),
                source,
            })?;
        file.sync_all().map_err(|source| LlmAuditSegmentError::Io {
            path: path.clone(),
            source,
        })?;
        restrict_private_file(&path).map_err(|source| LlmAuditSegmentError::Io {
            path: path.clone(),
            source,
        })?;
        Ok((id, file_name, file, FILE_HEADER_LEN))
    }

    fn segment_path(&self, file_name: &str) -> PathBuf {
        self.segments_dir.join(file_name)
    }
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn chrono_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db(path: &Path) -> Connection {
        let connection = Connection::open(path).expect("db");
        connection
            .execute_batch(
                "CREATE TABLE llm_audit_segments (
                    id INTEGER PRIMARY KEY,
                    file_name TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL,
                    closed_at TEXT,
                    byte_size INTEGER NOT NULL DEFAULT 0,
                    record_count INTEGER NOT NULL DEFAULT 0
                );",
            )
            .expect("schema");
        connection
    }

    #[test]
    fn append_and_read_roundtrip() {
        let dir = tempfile::tempdir().expect("temp");
        let db_path = dir.path().join("meta.sqlite");
        let connection = setup_db(&db_path);
        let store = LlmAuditSegmentStore::open(dir.path()).expect("store");
        let payload =
            r#"{"version":1,"format":"provider_request_v1","method":"POST","body":"hello"}"#;
        let locator = store
            .append_detail(&connection, LlmAuditDetailKind::Request, payload)
            .expect("append");
        let read = store.read_detail(&connection, &locator).expect("read");
        assert_eq!(read, payload);
        assert_eq!(locator.offset, FILE_HEADER_LEN);
    }

    #[test]
    fn append_multiple_records() {
        let dir = tempfile::tempdir().expect("temp");
        let db_path = dir.path().join("meta.sqlite");
        let connection = setup_db(&db_path);
        let store = LlmAuditSegmentStore::open(dir.path()).expect("store");
        let a = store
            .append_detail(&connection, LlmAuditDetailKind::Request, r#"{"a":1}"#)
            .expect("a");
        let b = store
            .append_detail(&connection, LlmAuditDetailKind::Response, r#"{"b":2}"#)
            .expect("b");
        assert_ne!(a.offset, b.offset);
        assert_eq!(
            store.read_detail(&connection, &a).expect("read a"),
            r#"{"a":1}"#
        );
        assert_eq!(
            store.read_detail(&connection, &b).expect("read b"),
            r#"{"b":2}"#
        );
    }
}
