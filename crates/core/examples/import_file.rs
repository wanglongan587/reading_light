use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow::anyhow!("usage: import_file <book.epub> [data-dir]"))?,
    );
    let data = PathBuf::from(args.next().unwrap_or_else(|| "target/import-check".into()));
    let mut store = glow_core::Store::open(data)?;
    let id = store.import_book(&path, None)?;
    let book = store.state.books.iter().find(|book| book.id == id).unwrap();
    println!(
        "IMPORT_OK id={} title={} chapters-ready",
        book.id, book.title
    );
    Ok(())
}
