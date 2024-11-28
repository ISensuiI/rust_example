use std::net::{IpAddr, Ipv4Addr};

use futures::{future, prelude::*};
use tarpc::{
  context,
  server::{self, incoming::Incoming, Channel},
  tokio_serde::formats::Json,
};

#[tarpc::service]
pub trait World {
  /// Returns a greeting for name.
  async fn hello(name: String) -> String;
}

// This is the type that implements the generated World trait. It is the business logic
// and is used to start the server.
#[derive(Clone)]
struct HelloWorldServer;

impl World for HelloWorldServer {
  async fn hello(self, _: context::Context, name: String) -> String {
    format!("Hello, {}!", name)
  }
}

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
  tokio::spawn(fut);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let server_addr = (IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

  let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Json::default).await?;
  listener.config_mut().max_frame_length(usize::MAX);
  listener
    // Ignore accept errors.
    .filter_map(|r| future::ready(r.ok()))
    .map(server::BaseChannel::with_defaults)
    // Limit channels to 1 per IP.
    .max_channels_per_key(1, |t| t.transport().peer_addr().unwrap().ip())
    // serve is generated by the service attribute. It takes as input any type implementing
    // the generated World trait.
    .map(|channel| {
      let server = HelloWorldServer;
      channel.execute(server.serve()).for_each(spawn)
    })
    // Max 10 channels.
    .buffer_unordered(10)
    .for_each(|_| async {})
    .await;

  Ok(())
}
