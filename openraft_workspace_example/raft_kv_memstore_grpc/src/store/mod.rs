use std::{
  collections::BTreeMap,
  fmt::Debug,
  sync::{Arc, Mutex},
};

use bincode;
use openraft::{
  Entry, EntryPayload, LogId, RaftSnapshotBuilder, SnapshotMeta, StorageError, StoredMembership,
  alias::SnapshotDataOf,
  storage::{RaftStateMachine, Snapshot},
};
use serde::{Deserialize, Serialize};

use crate::{NodeId, TypeConfig, protobuf::Response, typ};

pub type LogStore = memstore::LogStore<TypeConfig>;

#[derive(Debug)]
pub struct StoredSnapshot {
  pub meta: SnapshotMeta<TypeConfig>,

  /// The data of the state machine at the time of this snapshot.
  pub data: Box<typ::SnapshotData>,
}

/// Data contained in the Raft state machine.
///
/// Note that we are using `serde` to serialize the
/// `data`, which has a implementation to be serialized. Note that for this test we set both the key
/// and value as String, but you could set any type of value that has the serialization impl.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct StateMachineData {
  pub last_applied: Option<LogId<NodeId>>,

  pub last_membership: StoredMembership<TypeConfig>,

  /// Application data.
  pub data: BTreeMap<String, String>,
}

impl StateMachineData {
  pub fn to_bytes(&self) -> Vec<u8> {
    bincode::serialize(self).expect("Failed to serialize StateMachineData")
  }

  pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
    bincode::deserialize(bytes)
  }
}

/// Defines a state machine for the Raft cluster. This state machine represents a copy of the
/// data for this node. Additionally, it is responsible for storing the last snapshot of the data.
#[derive(Debug, Default)]
pub struct StateMachineStore {
  /// The Raft state machine.
  pub state_machine: Mutex<StateMachineData>,

  snapshot_idx: Mutex<u64>,

  /// The last received snapshot.
  current_snapshot: Mutex<Option<StoredSnapshot>>,
}

impl RaftSnapshotBuilder<TypeConfig> for Arc<StateMachineStore> {
  #[tracing::instrument(level = "trace", skip(self))]
  async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<TypeConfig>> {
    let data;
    let last_applied_log;
    let last_membership;

    {
      // Serialize the data of the state machine.
      let state_machine = self.state_machine.lock().unwrap().clone();

      last_applied_log = state_machine.last_applied;
      last_membership = state_machine.last_membership.clone();
      data = state_machine;
    }

    let snapshot_idx = {
      let mut l = self.snapshot_idx.lock().unwrap();
      *l += 1;
      *l
    };

    let snapshot_id = if let Some(last) = last_applied_log {
      format!("{}-{}-{}", last.leader_id, last.index, snapshot_idx)
    } else {
      format!("--{}", snapshot_idx)
    };

    let meta = SnapshotMeta {
      last_log_id: last_applied_log,
      last_membership,
      snapshot_id,
    };

    let snapshot = StoredSnapshot {
      meta: meta.clone(),
      data: Box::new(data.clone()),
    };

    {
      let mut current_snapshot = self.current_snapshot.lock().unwrap();
      *current_snapshot = Some(snapshot);
    }

    Ok(Snapshot {
      meta,
      snapshot: Box::new(data),
    })
  }
}

impl RaftStateMachine<TypeConfig> for Arc<StateMachineStore> {
  type SnapshotBuilder = Self;

  async fn applied_state(
    &mut self,
  ) -> Result<(Option<LogId<NodeId>>, StoredMembership<TypeConfig>), StorageError<TypeConfig>> {
    let state_machine = self.state_machine.lock().unwrap();
    Ok((
      state_machine.last_applied,
      state_machine.last_membership.clone(),
    ))
  }

  #[tracing::instrument(level = "trace", skip(self, entries))]
  async fn apply<I>(&mut self, entries: I) -> Result<Vec<Response>, StorageError<TypeConfig>>
  where
    I: IntoIterator<Item = Entry<TypeConfig>>,
  {
    let mut res = Vec::new(); //No `with_capacity`; do not know `len` of iterator

    let mut sm = self.state_machine.lock().unwrap();

    for entry in entries {
      tracing::debug!(%entry.log_id, "replicate to sm");

      sm.last_applied = Some(entry.log_id);

      match entry.payload {
        EntryPayload::Blank => res.push(Response { value: None }),
        EntryPayload::Normal(req) => {
          sm.data.insert(req.key, req.value.clone());
          res.push(Response {
            value: Some(req.value),
          });
        }
        EntryPayload::Membership(ref mem) => {
          sm.last_membership = StoredMembership::new(Some(entry.log_id), mem.clone());
          res.push(Response { value: None })
        }
      };
    }
    Ok(res)
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn begin_receiving_snapshot(
    &mut self,
  ) -> Result<Box<SnapshotDataOf<TypeConfig>>, StorageError<TypeConfig>> {
    Ok(Box::default())
  }

  #[tracing::instrument(level = "trace", skip(self, snapshot))]
  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMeta<TypeConfig>,
    snapshot: Box<SnapshotDataOf<TypeConfig>>,
  ) -> Result<(), StorageError<TypeConfig>> {
    tracing::info!("install snapshot");

    let new_snapshot = StoredSnapshot {
      meta: meta.clone(),
      data: snapshot,
    };

    // Update the state machine.
    {
      let updated_state_machine: StateMachineData = *new_snapshot.data.clone();
      let mut state_machine = self.state_machine.lock().unwrap();
      *state_machine = updated_state_machine;
    }

    // Update current snapshot.
    let mut current_snapshot = self.current_snapshot.lock().unwrap();
    *current_snapshot = Some(new_snapshot);
    Ok(())
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn get_current_snapshot(
    &mut self,
  ) -> Result<Option<Snapshot<TypeConfig>>, StorageError<TypeConfig>> {
    match &*self.current_snapshot.lock().unwrap() {
      Some(snapshot) => {
        let data = snapshot.data.clone();
        Ok(Some(Snapshot {
          meta: snapshot.meta.clone(),
          snapshot: data,
        }))
      }
      None => Ok(None),
    }
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }
}
