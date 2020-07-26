extern crate yaml_rust;

use super::constants::{APP_NAME, DIR_NAME};
use std::collections::HashMap;

use std::clone::Clone;
use std::fs;
use std::path::PathBuf;
use std::result::Result;
use yaml_rust::{Yaml, YamlLoader};

use log::info;

pub struct ServerConfig {
    pub port: u16,
    pub use_unix_domain_socket: bool,
    pub use_ssl: bool,
}

impl ServerConfig {
    fn new(port: u16, use_unix_domain_socket: bool, use_ssl: bool) -> ServerConfig {
        ServerConfig {
            port: port,
            use_unix_domain_socket: use_unix_domain_socket,
            use_ssl: use_ssl,
        }
    }
}

#[derive(Clone)]
pub struct CacheConfig {
    /// cache.m0cchi.net:3000
    pub host: String,
    /// https://contents.m0cchi.net
    pub origin: String,
    pub expire: u32,
    pub err_expire: u32,
    pub keep_cache_after_shutdown: bool,
}

impl CacheConfig {
    fn new(
        host: &str,
        origin: &str,
        expire: u32,
        err_expire: u32,
        keep_cache_after_shutdown: bool,
    ) -> CacheConfig {
        CacheConfig {
            host: host.to_string(),
            origin: origin.to_string(),
            expire: expire,
            err_expire: err_expire,
            keep_cache_after_shutdown: keep_cache_after_shutdown,
        }
    }
}

pub struct Config {
    pub server: ServerConfig,
    pub cache: HashMap<String, CacheConfig>,
}

fn i64tou32(i: i64) -> Result<u32, &'static str> {
    if i <= u32::max_value() as i64 && i >= 0 {
        Ok(i as u32)
    } else {
        Err("error!")
    }
}

fn collect_config_path(home: &PathBuf) -> Result<PathBuf, String> {
    let candidate = vec![
        format!(".{}{}", APP_NAME, ".yml"),
        format!(".{}{}", APP_NAME, ".yaml"),
        format!("{}/{}{}", DIR_NAME, APP_NAME, ".yml"),
        format!("{}/{}{}", DIR_NAME, APP_NAME, ".yaml"),
    ];
    for c in &candidate {
        let config_path = home.join(c);
        if config_path.exists() {
            return Ok(config_path);
        }
    }
    Err(format!("missing {:?}", candidate))
}

pub fn load_config_file(home: &PathBuf) -> Result<Config, String> {
    let config_path = match collect_config_path(home) {
        Ok(v) => v,
        Err(err) => {
            return Err(err.to_string());
        }
    };
    info!(
        "load config file: {}",
        config_path
            .to_str()
            .expect("error: can't load config file")
    );

    let yaml_text = fs::read_to_string(config_path).unwrap();
    let yaml = YamlLoader::load_from_str(yaml_text.as_str()).unwrap();
    let hosts = &yaml[0]["hosts"];
    let default = &hosts["default"];
    let mut cache_config: HashMap<String, CacheConfig> = HashMap::new();
    match hosts {
        Yaml::Hash(hosts) => {
            for (host, config) in hosts {
                let host = host.as_str().unwrap();
                if host != "default" {
                    let origin = match &config["origin"] {
                        Yaml::String(origin) => origin,
                        _ => {
                            return Err(format!("parse error:\n\thost:{}\n\tstep:origin", host));
                        }
                    };
                    let expire: u32 = match config["expire"] {
                        Yaml::Integer(expire) => match i64tou32(expire) {
                            Ok(v) => v,
                            Err(_) => {
                                return Err(format!(
                                    "parse error:\n\thost:{}\n\tstep:expire",
                                    host
                                ));
                            }
                        },
                        _ => match default["expire"] {
                            Yaml::Integer(expire) => match i64tou32(expire) {
                                Ok(v) => v,
                                Err(_) => {
                                    return Err(format!(
                                        "parse error:\n\thost:{}\n\tstep:expire",
                                        host
                                    ));
                                }
                            },
                            _ => 86400,
                        },
                    };
                    let err_expire: u32 = match config["err_expire"] {
                        Yaml::Integer(expire) => match i64tou32(expire) {
                            Ok(v) => v,
                            Err(_) => {
                                return Err(format!(
                                    "parse error:\n\thost:{}\n\tstep:err_expire",
                                    host
                                ));
                            }
                        },
                        _ => match default["err_expire"] {
                            Yaml::Integer(expire) => match i64tou32(expire) {
                                Ok(v) => v,
                                Err(_) => {
                                    return Err(format!(
                                        "parse error:\n\thost:{}\n\tstep:err_expire",
                                        host
                                    ));
                                }
                            },
                            _ => 300,
                        },
                    };
                    let keep_cache_after_shutdown = match config["keep_cache_after_shutdown"] {
                        Yaml::Boolean(keep_cache_after_shutdown) => keep_cache_after_shutdown,
                        _ => match default["keep_cache_after_shutdown"] {
                            Yaml::Boolean(keep_cache_after_shutdown) => keep_cache_after_shutdown,
                            _ => false,
                        },
                    };
                    let config = CacheConfig::new(
                        &host,
                        &origin,
                        expire,
                        err_expire,
                        keep_cache_after_shutdown,
                    );
                    cache_config.insert(host.to_string(), config);
                }
            }
        }
        _ => {}
    }

    let server_config = ServerConfig::new(0, false, false);
    let config = Config {
        server: server_config,
        cache: cache_config,
    };
    Ok(config)
}
