//! Explicit offline reset; no dotenv, bootstrap, network, or graph deletion.
use quarry_backend::{adapters::sqlite::client::SqliteClient, app::migrations::reset_development};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let flag = args.next();
    let path = args.next();
    if flag.as_deref() != Some(std::ffi::OsStr::new("--discard"))
        || path.is_none()
        || args.next().is_some()
    {
        return Err("usage: reset_development --discard <explicit-disposable-sqlite-path>; stop the backend and pair with a fresh isolated Helix store before restarting".into());
    }
    let path = path.ok_or("database path required")?;
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() || !path.is_file() {
        return Err("select an existing disposable database with an absolute path".into());
    }
    reset_development(&SqliteClient::open(path)?)?;
    println!("development SQLite records discarded; restart only with a fresh isolated Helix store and reload clients");
    Ok(())
}
