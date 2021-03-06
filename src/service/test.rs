#![cfg(test)]

use crate::kbucket;
use crate::{
    handler::Handler,
    kbucket::{KBucketsTable, NodeStatus},
    node_info::NodeContact,
    query_pool::{QueryId, QueryPool},
    rpc,
    service::{ActiveRequest, Service},
    Discv5ConfigBuilder,
};
use enr::{CombinedKey, Enr, EnrBuilder};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};
use tokio::sync::{mpsc, oneshot};

fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

fn build_service(
    local_enr: Arc<RwLock<Enr<CombinedKey>>>,
    enr_key: Arc<RwLock<CombinedKey>>,
    listen_socket: SocketAddr,
) -> Service {
    let config = Discv5ConfigBuilder::new()
        .executor(Box::new(crate::executor::TokioExecutor::default()))
        .build();
    // build the session service
    let (_handler_exit, handler_send, handler_recv) = Handler::spawn(
        local_enr.clone(),
        enr_key.clone(),
        listen_socket,
        config.clone(),
    )
    .unwrap();

    let kbuckets = Arc::new(RwLock::new(KBucketsTable::new(
        local_enr.read().node_id().into(),
        Duration::from_secs(60),
    )));

    // create the required channels
    let (_discv5_send, discv5_recv) = mpsc::channel(30);
    let (_exit_send, exit) = oneshot::channel();

    Service {
        local_enr,
        enr_key,
        kbuckets,
        queries: QueryPool::new(config.query_timeout),
        active_requests: Default::default(),
        active_nodes_responses: HashMap::new(),
        ip_votes: None,
        handler_send,
        handler_recv,
        handler_exit: Some(_handler_exit),
        ping_heartbeat: tokio::time::interval(config.ping_interval),
        discv5_recv,
        event_stream: None,
        exit,
        config,
    }
}

#[tokio::test]
async fn test_updating_connection_on_ping() {
    init();
    let enr_key1 = CombinedKey::generate_secp256k1();
    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    let enr = EnrBuilder::new("v4")
        .ip(ip.clone().into())
        .udp(10001)
        .build(&enr_key1)
        .unwrap();
    let ip2: IpAddr = "127.0.0.1".parse().unwrap();
    let enr_key2 = CombinedKey::generate_secp256k1();
    let enr2 = EnrBuilder::new("v4")
        .ip(ip2.clone().into())
        .udp(10002)
        .build(&enr_key2)
        .unwrap();

    let socket_addr = enr.udp_socket().unwrap();

    let mut service = build_service(
        Arc::new(RwLock::new(enr)),
        Arc::new(RwLock::new(enr_key1)),
        socket_addr,
    );
    // Set up service with one disconnected node
    let key = kbucket::Key::from(enr2.node_id());
    if let kbucket::Entry::Absent(entry) = service.kbuckets.write().entry(&key) {
        match entry.insert(enr2.clone(), NodeStatus::Disconnected) {
            kbucket::InsertResult::Inserted => {}
            kbucket::InsertResult::Full => {
                panic!("Can't be full");
            }
            kbucket::InsertResult::Pending { .. } => {}
        }
    }

    // Add a fake request
    let response = rpc::Response {
        id: 1,
        body: rpc::ResponseBody::Ping {
            enr_seq: 2,
            ip: ip2,
            port: 10002,
        },
    };

    service.active_requests.insert(
        1,
        ActiveRequest {
            contact: NodeContact::Enr(Box::new(enr2)),
            request_body: rpc::RequestBody::Ping { enr_seq: 2 },
            query_id: Some(QueryId(1)),
            callback: None,
        },
    );

    // Handle the ping and expect the disconnected Node to become connected
    service.handle_rpc_response(response);
    let buckets = service.kbuckets.read();
    let node = buckets.iter_ref().next().unwrap();
    assert_eq!(node.status, NodeStatus::Connected);
}
