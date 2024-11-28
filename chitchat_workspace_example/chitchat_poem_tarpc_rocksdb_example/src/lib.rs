#![allow(clippy::uninlined_format_args)]

use std::{fmt::Display, io::Cursor, net::SocketAddr, path::Path, sync::Arc, time::SystemTime};

use chitchat::{
  spawn_chitchat, transport::UdpTransport, Chitchat, ChitchatConfig, ChitchatId,
  FailureDetectorConfig,
};
use cool_id_generator::Size;
use openraft::Config;
// use openraft::TokioRuntime;
use poem::Server;
use tokio::{sync::Mutex, time::Duration};

use crate::{
  common::ChitchatApi,
  network::Network,
  store::{new_storage, Request, Response},
};

pub mod api_rpc;
pub mod chitchat_web_cmd;
pub mod client;
pub mod common;
pub mod network;
pub mod store;
pub mod web_openapi;
use std::net::{IpAddr, Ipv4Addr};

use futures::prelude::*;
use poem::{listener::TcpListener, Route};
use poem_openapi::OpenApiService;
use tarpc::{
  server::{self, incoming::Incoming, Channel},
  tokio_serde::formats::Json,
};

use crate::{
  api_rpc::World,
  common::{Api, Opt},
};
pub type NodeId = u64;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct Node {
  pub rpc_addr: String,
  pub api_addr: String,
}

impl Display for Node {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Node {{ rpc_addr: {}, api_addr: {} }}",
      self.rpc_addr, self.api_addr
    )
  }
}

pub type SnapshotData = Cursor<Vec<u8>>;

openraft::declare_raft_types!(
    pub TypeConfig:
        D = Request,
        R = Response,
        Node = Node,
);

pub mod typ {
  use openraft::error::Infallible;

  use crate::TypeConfig;

  pub type Entry = openraft::Entry<TypeConfig>;

  pub type RaftError<E = Infallible> = openraft::error::RaftError<TypeConfig, E>;
  pub type RPCError<E = Infallible> = openraft::error::RPCError<TypeConfig, RaftError<E>>;

  pub type ClientWriteError = openraft::error::ClientWriteError<TypeConfig>;
  pub type CheckIsLeaderError = openraft::error::CheckIsLeaderError<TypeConfig>;
  pub type ForwardToLeader = openraft::error::ForwardToLeader<TypeConfig>;
  pub type InitializeError = openraft::error::InitializeError<TypeConfig>;

  pub type ClientWriteResponse = openraft::raft::ClientWriteResponse<TypeConfig>;
}

pub type ExampleRaft = openraft::Raft<TypeConfig>;

pub async fn start_example_raft_node<P>(node_id: NodeId, dir: P, options: Opt) -> anyhow::Result<()>
where
  P: AsRef<Path>,
{
  let http_addr = options.clone().http_addr;
  let rpc_addr = options.clone().rpc_addr;
  // Create a configuration for the raft instance.
  let config = Config {
    heartbeat_interval: 250,
    election_timeout_min: 299,
    ..Default::default()
  };

  let config = Arc::new(config.validate().unwrap());

  let (log_store, state_machine_store) = new_storage(&dir).await;

  let kvs = state_machine_store.data.kvs.clone();

  // Create the network layer that will connect and communicate the raft instances and
  // will be used in conjunction with the store created above.
  let network = Network {};

  // Create a local raft instance.
  let raft = openraft::Raft::new(
    node_id,
    config.clone(),
    network,
    log_store,
    state_machine_store,
  )
  .await
  .unwrap();

  match start_chitchat(options.clone()).await {
    Ok(chitchat) => {
      let api = Api {
        id: node_id,
        api_addr: http_addr.clone(),
        rpc_addr: rpc_addr.clone(),
        raft: raft.clone(),
        key_values: kvs,
        config: config.clone(),
      };

      _ = start_tarpc(api.clone()).await;
      _ = start_poem(api, ChitchatApi { chitchat }, options).await;
      Ok(())
    }
    Err(other) => Err(other),
  }
}

async fn start_tarpc(api: Api) -> Result<(), std::io::Error> {
  let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), 12345);
  let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Json::default).await?;
  listener.config_mut().max_frame_length(usize::MAX);

  tokio::spawn(async move {
    listener
      // Ignore accept errors.
      .filter_map(|r| future::ready(r.ok()))
      .map(server::BaseChannel::with_defaults)
      // Limit channels to 1 per IP.
      .max_channels_per_key(1, |t| t.transport().peer_addr().unwrap().ip())
      // serve is generated by the service attribute. It takes as input any type implementing
      // the generated World trait.
      .map(|channel| {
        // let server = Api {
        //   num: num_clone.clone(),
        // };
        let api_clone = api.clone();
        channel.execute(api_clone.serve()).for_each(spawn)
      })
      // Max 10 channels.
      .buffer_unordered(10)
      .for_each(|_| async {})
      .await;
  });
  Ok(())
}

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
  tokio::spawn(fut);
}

async fn start_poem(
  api: Api,
  chitchat_api: ChitchatApi,
  options: Opt,
) -> Result<(), std::io::Error> {
  let api_service =
    OpenApiService::new(api, "Hello World", "1.0").server("http://localhost:3000/api");

  let app = Route::new().nest("/api", api_service);

  println!("access http://127.0.0.1:3000/api/hello");

  let server1 = poem::Server::new(TcpListener::bind("127.0.0.1:3000")).run(app);

  // let chitchat_api = { chitchat };
  let api_service = OpenApiService::new(chitchat_api, "Hello World", "1.0")
    .server(format!("http://{}/", options.listen_addr));
  let docs = api_service.swagger_ui();
  let app2 = Route::new().nest("/", api_service).nest("/docs", docs);
  let server2 = Server::new(TcpListener::bind(&options.listen_addr)).run(app2);

  _ = tokio::join!(server1, server2);
  Ok(())
}

async fn start_chitchat(opt: Opt) -> anyhow::Result<Arc<Mutex<Chitchat>>> {
  let public_addr = opt.public_addr.unwrap_or(opt.listen_addr);
  let node_id = opt
    .node_id
    .unwrap_or_else(|| generate_server_id(public_addr));
  let generation = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let chitchat_id = ChitchatId::new(node_id, generation, public_addr);
  let config = ChitchatConfig {
    cluster_id: "testing".to_string(),
    chitchat_id,
    gossip_interval: Duration::from_millis(opt.interval),
    listen_addr: opt.listen_addr,
    seed_nodes: opt.seeds.clone(),
    failure_detector_config: FailureDetectorConfig {
      dead_node_grace_period: Duration::from_secs(10),
      ..FailureDetectorConfig::default()
    },
    marked_for_deletion_grace_period: Duration::from_secs(60),
    catchup_callback: None,
    extra_liveness_predicate: None,
  };
  let chitchat_handler = spawn_chitchat(config, Vec::new(), &UdpTransport).await?;
  let chitchat = chitchat_handler.chitchat();
  Ok(chitchat)
}

fn generate_server_id(public_addr: SocketAddr) -> String {
  let cool_id = cool_id_generator::get_id(Size::Medium);
  format!("server:{public_addr}-{cool_id}")
}
