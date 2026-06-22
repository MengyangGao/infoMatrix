use std::path::Path;

use crate::SharedApiError;

pub fn default_db_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    Path::new(&home).join(".infomatrix").join("infomatrix.db").to_string_lossy().to_string()
}

pub fn ensure_parent_dir(path: &str) -> Result<(), SharedApiError> {
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
