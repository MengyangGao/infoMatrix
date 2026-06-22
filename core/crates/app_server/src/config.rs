use std::net::SocketAddr;

use shared_api::db::default_db_path;

#[derive(Clone)]
pub(crate) struct AppContext {
    pub(crate) db_path: String,
    pub(crate) user_agent: String,
    pub(crate) timeout_secs: u64,
    pub(crate) migrate_on_open: bool,
    pub(crate) api_token: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeConfig {
    pub(crate) db_path: String,
    pub(crate) bind_addr: SocketAddr,
    pub(crate) api_token: Option<String>,
}

pub(crate) fn resolve_runtime_config(
    args: impl IntoIterator<Item = String>,
) -> Result<RuntimeConfig, Box<dyn std::error::Error>> {
    resolve_runtime_config_with_env(args, |name| std::env::var(name).ok())
}

pub(crate) fn resolve_runtime_config_with_env(
    args: impl IntoIterator<Item = String>,
    get_env: impl Fn(&str) -> Option<String>,
) -> Result<RuntimeConfig, Box<dyn std::error::Error>> {
    let mut db_path = get_env("INFOMATRIX_DB_PATH").unwrap_or_else(default_db_path);
    let mut bind_addr = get_env("INFOMATRIX_BIND_ADDR")
        .and_then(|value| value.parse::<SocketAddr>().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 3199)));

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--db-path" => {
                let value = iter.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "--db-path requires a value",
                    )
                })?;
                db_path = value;
            }
            "--bind-addr" => {
                let value = iter.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "--bind-addr requires a value",
                    )
                })?;
                bind_addr = value.parse::<SocketAddr>()?;
            }
            "--port" => {
                let value = iter.next().ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "--port requires a value")
                })?;
                let port = value.parse::<u16>()?;
                bind_addr = SocketAddr::from(([127, 0, 0, 1], port));
            }
            _ => {}
        }
    }

    let allow_remote_bind = get_env("INFOMATRIX_ALLOW_REMOTE_BIND").as_deref() == Some("1");
    let api_token = get_env("INFOMATRIX_API_TOKEN").filter(|value| !value.trim().is_empty());
    if !bind_addr.ip().is_loopback() {
        if !allow_remote_bind {
            return Err("non-loopback bind requires INFOMATRIX_ALLOW_REMOTE_BIND=1".into());
        }
        if api_token.is_none() {
            return Err("non-loopback bind requires INFOMATRIX_API_TOKEN".into());
        }
    }

    Ok(RuntimeConfig { db_path, bind_addr, api_token })
}
