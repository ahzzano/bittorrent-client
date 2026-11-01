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
