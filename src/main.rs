extern crate env_logger as logger;
extern crate hyper;
extern crate hyper_tls;
extern crate log;
extern crate url;

use schnecke::config::{load_config_file, CacheConfig};
use schnecke::constants::TMP_NAME;
use std::collections::HashMap;
use std::convert::Infallible;

use hyper_tls::HttpsConnector;
use std::env;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process;
use url::Url;

use log::{debug, error, info};

use hyper::header::HeaderValue;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};

fn init_working_dir(home: &PathBuf) {
    let tmp_dir = home.join(TMP_NAME);
    if tmp_dir.exists() {
        let tmp_dir = tmp_dir.to_str().unwrap();
        match fs::remove_dir_all(tmp_dir) {
            Ok(_) => {}
            Err(err) => {
                error!("{}:{}", err, tmp_dir);
                process::exit(1);
            }
        }
    }
    match fs::create_dir_all(tmp_dir) {
        Ok(_) => {}
        Err(err) => {
            error!("{}", err);
            process::exit(1);
        }
    }
}

fn write_port(home: &PathBuf, port: u16) {
    let tmp_dir = home.join(TMP_NAME);
    if !tmp_dir.exists() {
        error!("missing {:?}", tmp_dir.as_os_str());
        process::exit(1);
    }
    let mut writer = BufWriter::new(fs::File::create(tmp_dir.join("port.tmp")).unwrap());
    writer.write(port.to_string().as_bytes()).unwrap();
}

async fn proxy(
    client: hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>,
    req: Request<Body>,
    config: HashMap<String, CacheConfig>,
) -> Result<Response<Body>, Infallible> {
    let host = &req.headers()["host"];
    let host = &host.to_str().unwrap().to_string();
    let parts: Vec<&str> = host.split(':').collect();
    let config = match config.get(parts[0]) {
        Some(v) => v,
        None => {
            return Ok(Response::new(Body::from("missing")));
        }
    };

    let method = &req.method();
    let origin = &config.origin;
    let mut uri = Url::parse(origin).unwrap();
    info!("{:?}", uri);
    uri.set_path(req.uri().path());
    uri.set_query(req.uri().query());
    let origin_domain = match uri.domain() {
        Some(v) => v,
        None => {
            return Ok(Response::new(Body::from("missing")));
        }
    };

    let mut client_req = Request::builder()
        .method(method.as_str())
        .uri(uri.to_string());

    let headers = client_req.headers_mut().unwrap();
    for (_, (name, value)) in req.headers().iter().enumerate() {
        // to lowercase
        if name == "host" {
            headers.insert(name, HeaderValue::from_str(origin_domain).unwrap());
        } else {
            headers.insert(name, value.clone());
        }
    }

    let client_req = client_req.body(Body::empty()).unwrap();

    debug!("client_req {:?}", client_req);
    debug!("body {:?}", req.body());

    debug!("{:?}", &req.headers());
    debug!("{:?}", req.uri());

    match client.request(client_req).await {
        Ok(value) => Ok(value),
        Err(err) => {
            debug!("{:?}", err);
            Ok(Response::new(Body::from(err.to_string())))
        }
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match env::var("RUST_LOG") {
        Ok(_) => {}
        Err(_) => {
            env::set_var("RUST_LOG", "info");
        }
    }
    logger::init();
    let home = match env::var("HOME") {
        Ok(v) => PathBuf::from(v),
        Err(err) => {
            error!("{}", err);
            error!("env HOME=workding directory ./schnecke");
            process::exit(1);
        }
    };
    let config = match load_config_file(&home) {
        Ok(v) => v,
        Err(err) => {
            error!("{}", err);
            process::exit(1);
        }
    };
    let cache_config = config.cache;

    for (_, config) in &cache_config {
        info!("{} -> {}", config.host, config.origin);
    }

    init_working_dir(&home);

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    let make_svc = make_service_fn(move |_| {
        let client = client.clone();
        let cache_config = cache_config.clone();
        async {
            Ok::<_, Infallible>(service_fn(move |req| {
                proxy(client.clone(), req, cache_config.clone())
            }))
        }
    });

    let addr = ([127, 0, 0, 1], config.server.port).into();
    let server = Server::bind(&addr).serve(make_svc);
    let addr = server.local_addr();
    write_port(&home, addr.port());
    info!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}
