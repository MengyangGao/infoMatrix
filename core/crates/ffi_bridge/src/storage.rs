use app_core::InfoMatrixAppCore;
use shared_api::db::{default_db_path, ensure_parent_dir};
use storage::Storage;

pub fn open_storage(db_path: &Option<String>) -> Result<Storage, String> {
    let db_path = db_path.clone().unwrap_or_else(default_db_path);
    ensure_parent_dir(&db_path).map_err(|err| err.to_string())?;

    let storage = Storage::open(&db_path).map_err(|err| err.to_string())?;
    storage.migrate().map_err(|err| err.to_string())?;
    Ok(storage)
}

pub fn open_app_core(db_path: &Option<String>) -> Result<InfoMatrixAppCore, String> {
    Ok(InfoMatrixAppCore::new(open_storage(db_path)?))
}
