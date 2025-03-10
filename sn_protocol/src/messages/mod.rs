// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! Data messages and their possible responses.
mod cmd;
mod node_id;
mod query;
mod register;
mod response;

pub use self::{
    cmd::{Cmd, Hash},
    node_id::NodeId,
    query::Query,
    register::RegisterCmd,
    response::{CmdOk, CmdResponse, QueryResponse},
};

use super::NetworkAddress;

use serde::{Deserialize, Serialize};

#[allow(clippy::large_enum_variant)]
/// A request to peers in the network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// A cmd sent to peers. Cmds are writes, i.e. can cause mutation.
    Cmd(Cmd),
    /// A query sent to peers. Queries are read-only.
    Query(Query),
}

/// A response to peers in the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// The response to a cmd.
    Cmd(CmdResponse),
    /// The response to a query.
    Query(QueryResponse),
}

impl Request {
    /// Used to send a request to the close group of the address.
    pub fn dst(&self) -> NetworkAddress {
        match self {
            Request::Cmd(cmd) => cmd.dst(),
            Request::Query(query) => query.dst(),
        }
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
