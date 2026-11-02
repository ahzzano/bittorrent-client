use crate::{
    bencode::{self, Value},
    metainfo::MetainfoError::{InvalidFieldType, InvalidShape, MissingField, ParserError},
};

/// Contents of a .torrent (metainfo) file, as defined by BEP 3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metainfo {
    /// Tracker URL.
    pub announce: String,
    /// Tiers of tracker URLs (BEP 12).
    pub announce_list: Option<Vec<Vec<String>>>,
    /// Creation time in Unix seconds.
    pub creation_date: Option<i64>,
    pub comment: Option<String>,
    pub created_by: Option<String>,
    pub info: Info,
    /// SHA-1 of the raw bencoded `info` dictionary bytes, as they appear in the file.
    pub info_hash: [u8; 20],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// File name (single-file) or root directory name (multi-file).
    pub name: String,
    /// Number of bytes per piece.
    pub piece_length: u64,
    /// SHA-1 hash of each piece.
    pub pieces: Vec<[u8; 20]>,
    /// Private torrent flag (BEP 27).
    pub private: bool,
    pub files: FileLayout,
}

impl From<std::io::Error> for MetainfoError {
    fn from(value: std::io::Error) -> Self {
        MetainfoError::IOError(value)
    }
}

impl From<bencode::ParserError> for MetainfoError {
    fn from(value: bencode::ParserError) -> Self {
        ParserError(value)
    }
}

/// The info dictionary has either `length` or `files`, never both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileLayout {
    Single { length: u64 },
    Multi { files: Vec<FileEntry> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub length: u64,
    /// Path components relative to `Info::name`.
    pub path: Vec<String>,
}

pub enum MetainfoError {
    IOError(std::io::Error),
    ParserError(bencode::ParserError),
    InvalidShape,
    MissingField(String),
    InvalidFieldType(String),
}

fn field<'v, 'a>(dict: &'v Value<'a>, key: &str) -> Result<&'v Value<'a>, MetainfoError> {
    dict.get(key.as_bytes())
        .ok_or_else(|| MissingField(key.to_string()))
}

fn req_str<'a>(dict: &Value<'a>, key: &str) -> Result<&'a str, MetainfoError> {
    field(dict, key)?
        .as_str()
        .ok_or_else(|| InvalidFieldType(key.to_string()))
}

fn req_int(dict: &Value<'_>, key: &str) -> Result<i64, MetainfoError> {
    field(dict, key)?
        .as_int()
        .ok_or_else(|| InvalidFieldType(key.to_string()))
}

fn req_u64(dict: &Value<'_>, key: &str) -> Result<u64, MetainfoError> {
    u64::try_from(req_int(dict, key)?).map_err(|_| InvalidFieldType(key.to_string()))
}

fn req_list<'v, 'a>(dict: &'v Value<'a>, key: &str) -> Result<&'v [Value<'a>], MetainfoError> {
    field(dict, key)?
        .as_list()
        .ok_or_else(|| InvalidFieldType(key.to_string()))
}

/// Path components that refer to the current or parent directory, or to nothing at all.
const RESTRICTED_COMPONENTS: &[&str] = &["", ".", ".."];

/// Characters that would split one component into several once joined into a path.
const PATH_SEPARATORS: [char; 2] = ['/', '\\'];

/// Rejects names that could escape the download directory once joined into a path.
fn safe_component(s: &str, key: &str) -> Result<String, MetainfoError> {
    if RESTRICTED_COMPONENTS.contains(&s) || s.contains(PATH_SEPARATORS) {
        return Err(InvalidFieldType(key.to_string()));
    }
    Ok(s.to_string())
}

impl FileEntry {
    fn new(entry: &Value) -> Result<Self, MetainfoError> {
        if entry.as_dict().is_none() {
            return Err(InvalidFieldType("files".to_string()));
        }

        let path = req_list(entry, "path")?
            .iter()
            .map(|c| {
                let s = c
                    .as_str()
                    .ok_or_else(|| InvalidFieldType("path".to_string()))?;
                safe_component(s, "path")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if path.is_empty() {
            return Err(InvalidFieldType("path".to_string()));
        }

        Ok(FileEntry {
            length: req_u64(entry, "length")?,
            path,
        })
    }
}

impl Info {
    pub fn new(dict: &Value) -> Result<Self, MetainfoError> {
        let name = safe_component(req_str(dict, "name")?, "name")?;

        let piece_length = req_u64(dict, "piece length")?;
        if piece_length == 0 {
            return Err(InvalidFieldType("piece length".to_string()));
        }

        let pieces = field(dict, "pieces")?
            .as_bytes()
            .ok_or_else(|| InvalidFieldType("pieces".to_string()))?;
        let (pieces, rest) = pieces.as_chunks::<20>();
        if !rest.is_empty() {
            return Err(InvalidFieldType("pieces".to_string()));
        }

        let files = match (dict.get(b"length"), dict.get(b"files")) {
            (Some(_), None) => FileLayout::Single {
                length: req_u64(dict, "length")?,
            },
            (None, Some(_)) => {
                let files = req_list(dict, "files")?
                    .iter()
                    .map(FileEntry::new)
                    .collect::<Result<Vec<_>, _>>()?;
                if files.is_empty() {
                    return Err(InvalidFieldType("files".to_string()));
                }
                FileLayout::Multi { files }
            }
            (None, None) => return Err(MissingField("length".to_string())),
            (Some(_), Some(_)) => return Err(InvalidShape),
        };

        Ok(Info {
            name,
            piece_length,
            pieces: pieces.to_vec(),
            private: dict.get(b"private").and_then(Value::as_int) == Some(1),
            files,
        })
    }

    pub fn hash(&self) -> [u8; 20] {
        [0; 20]
    }
}

impl Metainfo {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MetainfoError> {
        let mut parser = bencode::Parser::new(bytes);
        let result = parser.parse()?;

        if result.as_dict().is_none() {
            return Err(MetainfoError::InvalidShape);
        }

        let announce = req_str(&result, "announce")?.to_string();

        let info = field(&result, "info")?;
        if info.as_dict().is_none() {
            return Err(InvalidFieldType("info".to_string()));
        }

        let info = {
            let this = Info::new(info);
            match this {
                Ok(t) => t,
                Err(e) => return Err(InvalidShape),
            }
        };

        let info_hash = info.hash();

        // Get non list keys
        Ok(Metainfo {
            announce: announce,
            announce_list: None,
            creation_date: None,
            comment: None,
            created_by: None,
            info,
            info_hash,
        })
    }

    pub fn from_file(path: &str) -> Result<Self, MetainfoError> {
        let bytes = std::fs::read(path)?;

        Metainfo::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(input: &[u8]) -> Result<Info, MetainfoError> {
        Info::new(
            &bencode::Parser::new(input)
                .parse()
                .ok()
                .expect("valid bencode"),
        )
    }

    #[test]
    fn parses_single_file_info() {
        let input = [
            &b"d6:lengthi5e4:name1:a12:piece lengthi16e6:pieces40:"[..],
            &[1u8; 20],
            &[2u8; 20],
            b"7:privatei1ee",
        ]
        .concat();
        let Ok(info) = info(&input) else {
            panic!("expected valid info");
        };
        assert_eq!(info.name, "a");
        assert_eq!(info.piece_length, 16);
        assert_eq!(info.pieces, vec![[1u8; 20], [2u8; 20]]);
        assert!(info.private);
        assert_eq!(info.files, FileLayout::Single { length: 5 });
    }

    #[test]
    fn parses_multi_file_info() {
        let input =
            b"d5:filesld6:lengthi3e4:pathl1:d1:feee4:name4:root12:piece lengthi16e6:pieces0:e";
        let Ok(info) = info(input) else {
            panic!("expected valid info");
        };
        assert!(!info.private);
        assert_eq!(
            info.files,
            FileLayout::Multi {
                files: vec![FileEntry {
                    length: 3,
                    path: vec!["d".to_string(), "f".to_string()],
                }],
            }
        );
    }

    #[test]
    fn rejects_bad_pieces_length() {
        let input = b"d6:lengthi5e4:name1:a12:piece lengthi16e6:pieces3:abce";
        assert!(matches!(info(input), Err(InvalidFieldType(k)) if k == "pieces"));
    }

    #[test]
    fn rejects_negative_length() {
        let input = b"d6:lengthi-5e4:name1:a12:piece lengthi16e6:pieces0:e";
        assert!(matches!(info(input), Err(InvalidFieldType(k)) if k == "length"));
    }

    #[test]
    fn rejects_both_and_neither_layouts() {
        let both = b"d5:filesle6:lengthi5e4:name1:a12:piece lengthi16e6:pieces0:e";
        assert!(matches!(info(both), Err(InvalidShape)));
        let neither = b"d4:name1:a12:piece lengthi16e6:pieces0:e";
        assert!(matches!(info(neither), Err(MissingField(k)) if k == "length"));
    }

    #[test]
    fn rejects_path_traversal() {
        let input =
            b"d5:filesld6:lengthi3e4:pathl2:..1:feee4:name4:root12:piece lengthi16e6:pieces0:e";
        assert!(matches!(info(input), Err(InvalidFieldType(k)) if k == "path"));
        let input = b"d6:lengthi5e4:name2:..12:piece lengthi16e6:pieces0:e";
        assert!(matches!(info(input), Err(InvalidFieldType(k)) if k == "name"));
    }

    #[test]
    fn rejects_missing_name() {
        let input = b"d6:lengthi5e12:piece lengthi16e6:pieces0:e";
        assert!(matches!(info(input), Err(MissingField(k)) if k == "name"));
    }
}
