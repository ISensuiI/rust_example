use bytes::Bytes;
use http_body_util::Full;
use hyper::http::StatusCode;
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper::{body::Incoming as IncomingBody, Method, Request, Response};
use hyper_util::rt::TokioIo;
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

const METRICS_CONTENT_TYPE: &str = "application/openmetrics-text;charset=utf-8;version=1.0.0";

pub(crate) async fn metrics_server(registry: Registry) -> Result<(), std::io::Error> {
  // Serve on localhost.
  let addr: SocketAddr = ([127, 0, 0, 1], 0).into();

  let make_metrics_service = MakeMetricService::new(registry);
  let listener = TcpListener::bind(addr).await?;
  loop {
    let (stream, _) = listener.accept().await?;
    let io = TokioIo::new(stream);
    let make_metrics_service_clone = make_metrics_service.clone();
    tokio::task::spawn(async move {
      if let Err(err) = http1::Builder::new()
        .serve_connection(io, make_metrics_service_clone)
        .await
      {
        tracing::error!("server error: {}", err);
      }
    });
  }
}

type SharedRegistry = Arc<Mutex<Registry>>;

#[derive(Debug, Clone)]
pub(crate) struct MetricService {
  reg: SharedRegistry,
}

impl MetricService {
  fn get_reg(&mut self) -> SharedRegistry {
    Arc::clone(&self.reg)
  }
  fn respond_with_metrics(&mut self) -> Response<Full<Bytes>> {
    let mut response: Response<Full<Bytes>> = Response::default();

    response.headers_mut().insert(
      hyper::header::CONTENT_TYPE,
      METRICS_CONTENT_TYPE.try_into().unwrap(),
    );

    let reg = self.get_reg();
    let mut inner_str = String::new();
    encode(&mut inner_str, &reg.lock().unwrap()).unwrap();
    *response.body_mut() = Full::new(Bytes::from(inner_str));

    *response.status_mut() = StatusCode::OK;

    response
  }

  fn respond_with_404_not_found(&mut self) -> Response<Full<Bytes>> {
    Response::builder()
      .status(StatusCode::NOT_FOUND)
      .body(Full::new(Bytes::from(
        "Not found try localhost:[port]/metrics".to_string(),
      )))
      .unwrap()
  }
}

impl Service<Request<IncomingBody>> for MetricService {
  type Response = Response<Full<Bytes>>;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn call(&self, req: Request<IncomingBody>) -> Self::Future {
    let req_path = req.uri().path();
    let req_method = req.method();
    let resp = if (req_method == Method::GET) && (req_path == "/metrics") {
      // Encode and serve metrics from registry.
      self.clone().respond_with_metrics()
    } else {
      self.clone().respond_with_404_not_found()
    };
    Box::pin(async { Ok(resp) })
  }
}

#[derive(Debug, Clone)]
pub(crate) struct MakeMetricService {
  reg: SharedRegistry,
}

impl MakeMetricService {
  pub(crate) fn new(registry: Registry) -> MakeMetricService {
    MakeMetricService {
      reg: Arc::new(Mutex::new(registry)),
    }
  }
}

impl Service<Request<IncomingBody>> for MakeMetricService {
  type Response = Response<Full<Bytes>>;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn call(&self, req: Request<IncomingBody>) -> Self::Future {
    let reg = self.reg.clone();
    let fut = async move { Ok(MetricService { reg }.call(req).await.unwrap()) };
    Box::pin(fut)
  }
}
