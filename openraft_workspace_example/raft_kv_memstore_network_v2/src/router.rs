use std::{
  collections::BTreeMap,
  sync::{Arc, Mutex},
};

use openraft::error::Unreachable;
use tokio::sync::oneshot;

use crate::{app::RequestTx, decode, encode, typ::RaftError, NodeId};

/// Simulate a network router.
#[derive(Debug, Clone, Default)]
pub struct Router {
  pub targets: Arc<Mutex<BTreeMap<NodeId, RequestTx>>>,
}

impl Router {
  /// Send request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
  pub async fn send<Req, Resp>(&self, to: NodeId, path: &str, req: Req) -> Result<Resp, Unreachable>
  where
    Req: serde::Serialize,
    Result<Resp, RaftError>: serde::de::DeserializeOwned,
  {
    let (resp_tx, resp_rx) = oneshot::channel();

    let encoded_req = encode(req);
    tracing::debug!("send to: {}, {}, {}", to, path, encoded_req);

    {
      let mut targets = self.targets.lock().unwrap();
      let tx = targets.get_mut(&to).unwrap();

      tx.send((path.to_string(), encoded_req, resp_tx)).unwrap();
    }

    let resp_str = resp_rx.await.unwrap();
    tracing::debug!("resp from: {}, {}, {}", to, path, resp_str);

    let res = decode::<Result<Resp, RaftError>>(&resp_str);
    res.map_err(|e| Unreachable::new(&e))
  }
}
