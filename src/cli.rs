use std::env;

use bittorrent_client::metainfo::Metainfo;

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: bt <file.torrent>");
        std::process::exit(1);
    };

    let Ok(info) = Metainfo::from_file(&path) else {
        eprintln!("Unable to parse error");
        std::process::exit(1);
    };

    println!("{info:?}")
}
