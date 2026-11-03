use bittorrent_client::metainfo::{FileLayout, Metainfo, MetainfoError};

#[test]
fn loads_sample_torrent() {
    let m = Metainfo::from_file("tests/sample.torrent").unwrap();

    assert_eq!(m.announce, "http://localhost:6969/announce");
    assert_eq!(m.info.name, "sample.txt");
    assert_eq!(m.info.piece_length, 32768);
    assert_eq!(m.info.pieces.len(), 3);
    assert_eq!(m.info.files, FileLayout::Single { length: 70000 });
}

#[test]
fn rejects_truncated_pieces() {
    let err = Metainfo::from_file("tests/bad.torrent").unwrap_err();

    assert!(matches!(err, MetainfoError::InvalidFieldType(f) if f == "pieces"));
}
