use axum::response::IntoResponse;
use axum::routing::*;
use axum::Router;
use std::net::SocketAddr;

use futures::prelude::*;
use tokio_stream::StreamExt;

use axum::body::Bytes;
use axum_streams::*;

#[derive(Clone, prost::Message)]
struct MyTestStructure {
    #[prost(string, tag = "1")]
    some_test_field: String,
}

fn source_test_stream() -> impl Stream<Item = MyTestStructure> {
    // Simulating a stream with a plain vector and throttling to show how it works
    stream::iter(vec![
        MyTestStructure {
            some_test_field: "test1".to_string()
        };
        1
    ])
    .throttle(std::time::Duration::from_millis(50))
}

async fn test_protobuf_stream() -> impl IntoResponse {
    StreamBodyAs::protobuf(source_test_stream())
}

async fn echo_handler(input: Bytes) -> impl IntoResponse {
    input
}

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(echo_handler))
        .route("/protobuf-stream", get(test_protobuf_stream));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
