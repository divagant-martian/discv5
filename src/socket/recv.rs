//! This is a standalone task that handles UDP packets as they are received.
//!
//! Every UDP packet passes a filter before being processed.

use super::filter::{Filter, FilterConfig};
use crate::packet::*;
use crate::Executor;
use log::{debug, trace};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub(crate) const MAX_PACKET_SIZE: usize = 1280;

/// The object sent back by the Recv handler.
pub struct InboundPacket {
    /// The originating socket addr.
    pub src: SocketAddr,
    /// The decoded packet.
    pub packet: Packet,
}

/// Convenience objects for setting up the recv handler.
pub struct RecvHandlerConfig {
    pub filter_config: FilterConfig,
    pub executor: Box<dyn Executor>,
    pub recv: tokio::net::udp::RecvHalf,
    pub whoareyou_magic: [u8; MAGIC_LENGTH],
    pub expected_responses: Arc<RwLock<HashMap<SocketAddr, usize>>>,
}

/// The main task that handles inbound UDP packets.
pub(crate) struct RecvHandler {
    /// The UDP recv socket.
    recv: tokio::net::udp::RecvHalf,
    /// The list of waiting responses. These are used to allow incoming packets from sources
    /// that we are expected a response from bypassing the rate-limit filters.
    expected_responses: Arc<RwLock<HashMap<SocketAddr, usize>>>,
    /// The packet filter which decides whether to accept or reject inbound packets.
    filter: Filter,
    /// The buffer to accept inbound datagrams.
    recv_buffer: [u8; MAX_PACKET_SIZE],
    /// WhoAreYou Magic Value. Used to decode raw WHOAREYOU packets.
    whoareyou_magic: [u8; MAGIC_LENGTH],
    /// The channel to send the packet handler.
    handler: mpsc::Sender<InboundPacket>,
    /// Exit channel to shutdown the recv handler.
    exit: oneshot::Receiver<()>,
}

impl RecvHandler {
    /// Spawns the `RecvHandler` on a provided executor.
    pub(crate) fn spawn(
        config: RecvHandlerConfig,
    ) -> (mpsc::Receiver<InboundPacket>, oneshot::Sender<()>) {
        let (exit_sender, exit) = oneshot::channel();

        // create the channel to send decoded packets to the handler
        let (handler, handler_recv) = mpsc::channel(30);

        let mut recv_handler = RecvHandler {
            recv: config.recv,
            filter: Filter::new(&config.filter_config),
            recv_buffer: [0; MAX_PACKET_SIZE],
            whoareyou_magic: config.whoareyou_magic,
            expected_responses: config.expected_responses,
            handler,
            exit,
        };

        // start the handler
        config.executor.spawn(Box::pin(async move {
            debug!("Recv handler starting");
            recv_handler.start().await;
        }));
        (handler_recv, exit_sender)
    }

    /// The main future driving the recv handler. This will shutdown when the exit future is fired.
    async fn start(&mut self) {
        loop {
            tokio::select! {
                Ok((length, src)) = self.recv.recv_from(&mut self.recv_buffer) => {
                    self.handle_inbound(src, length).await;
                }
                _ = &mut self.exit => {
                    debug!("Recv handler shutdown");
                    return;
                }
            }
        }
    }

    /// Handles in incoming packet. Passes through the filter, decodes and sends to the packet
    /// handler.
    async fn handle_inbound(&mut self, src: SocketAddr, length: usize) {
        println!("RECV: Handling inbound packet");
        // Permit all expected responses
        let permitted = self.expected_responses.read().get(&src).is_some();

        // Perform the first run of the filter. This checks for rate limits and black listed IP
        // addresses.
        if !permitted && !self.filter.initial_pass(&src) {
            trace!("Packet filtered from source: {:?}", src);
            return;
        }
        // Decodes the packet
        let packet = match Packet::decode(&self.recv_buffer[..length], &self.whoareyou_magic) {
            Ok(p) => p,
            Err(e) => {
                debug!("Packet decoding failed: {:?}", e); // could not decode the packet, drop it
                return;
            }
        };

        // Perform packet-level filtering
        if !permitted && !self.filter.final_pass(&src, &packet) {
            return;
        }

        let inbound = InboundPacket { src, packet };

        // send the filtered decoded packet to the handler.
        self.handler.send(inbound).await.unwrap_or_else(|_| ());
        println!("RECV: Handling inbound packet complete");
    }
}
