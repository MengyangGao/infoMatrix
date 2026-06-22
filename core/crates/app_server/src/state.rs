use app_core::InfoMatrixAppCore;
use reqwest::Client;
use storage::Storage;

use crate::config::AppContext;
use crate::error::ApiError;

pub(crate) fn build_http_client(context: &AppContext) -> Result<Client, ApiError> {
    Client::builder()
        .user_agent(context.user_agent.clone())
        .timeout(std::time::Duration::from_secs(context.timeout_secs))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .map_err(|err| ApiError::Internal(err.to_string()))
}

pub(crate) fn open_storage(context: &AppContext) -> Result<Storage, ApiError> {
    let storage = Storage::open(&context.db_path)?;
    if context.migrate_on_open {
        storage.migrate()?;
    }
    Ok(storage)
}

pub(crate) fn open_app_core(context: &AppContext) -> Result<InfoMatrixAppCore, ApiError> {
    Ok(InfoMatrixAppCore::new(open_storage(context)?))
}
