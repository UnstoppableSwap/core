use crate::network::quote::BidQuote;
use crate::network::rendezvous::XmrBtcNamespace;
use crate::network::{quote, swarm};
use crate::protocol::Database;
use anyhow::Result;
use arti_client::TorClient;
use futures::StreamExt;
use libp2p::multiaddr::Protocol;
use libp2p::request_response;
use libp2p::swarm::dial_opts::DialOpts;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identity, ping, rendezvous, Multiaddr, PeerId, Swarm};
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tor_rtcompat::tokio::TokioRustlsRuntime;
use typeshare::typeshare;

/// Returns sorted list of sellers, with [Online](Status::Online) listed first.
///
/// First uses the rendezvous node to discover peers in the given namespace,
/// then fetches a quote from each peer that was discovered. If fetching a quote
/// from a discovered peer fails the seller's status will be
/// [Unreachable](Status::Unreachable).
///
/// If a database is provided, it will be used to get the list of peers that
/// have already been discovered previously and attempt to fetch a quote from them.
pub async fn list_sellers(
    rendezvous_points: Vec<(PeerId, Multiaddr)>,
    namespace: XmrBtcNamespace,
    maybe_tor_client: Option<Arc<TorClient<TokioRustlsRuntime>>>,
    identity: identity::Keypair,
    db: Option<Arc<dyn Database + Send + Sync>>,
) -> Result<Vec<SellerStatus>> {
    let behaviour = Behaviour {
        rendezvous: rendezvous::client::Behaviour::new(identity.clone()),
        quote: quote::cli(),
        ping: ping::Behaviour::new(ping::Config::new().with_timeout(Duration::from_secs(60))),
    };
    let swarm = swarm::cli(identity, maybe_tor_client, behaviour).await?;

    // If a database is passed in: Fetch all peer addresses from the database and fetch quotes from them
    let external_dial_queue = match db {
        Some(db) => {
            let peers = db.get_all_peer_addresses().await?;
            VecDeque::from(peers)
        }
        None => VecDeque::new(),
    };

    let event_loop = EventLoop::new(swarm, rendezvous_points, namespace, external_dial_queue);
    let sellers = event_loop.run().await;

    Ok(sellers)
}

#[serde_as]
#[typeshare]
#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone, Ord, PartialOrd)]
pub struct QuoteWithAddress {
    /// The multiaddr of the seller (at which we were able to connect to and get the quote from)
    #[serde_as(as = "DisplayFromStr")]
    #[typeshare(serialized_as = "string")]
    pub multiaddr: Multiaddr,

    /// The peer id of the seller
    #[typeshare(serialized_as = "string")]
    pub peer_id: PeerId,

    /// The quote of the seller
    pub quote: BidQuote,
}

#[typeshare]
#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone, Ord, PartialOrd)]
pub struct UnreachableSeller {
    /// The peer id of the seller
    #[typeshare(serialized_as = "string")]
    pub peer_id: PeerId,
}

#[typeshare]
#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone, Ord, PartialOrd)]
#[serde(tag = "type", content = "content")]
pub enum SellerStatus {
    Online(QuoteWithAddress),
    Unreachable(UnreachableSeller),
}

#[allow(unused)]
#[derive(Debug)]
enum OutEvent {
    Rendezvous(rendezvous::client::Event),
    Quote(quote::OutEvent),
    Ping(ping::Event),
}

impl From<rendezvous::client::Event> for OutEvent {
    fn from(event: rendezvous::client::Event) -> Self {
        OutEvent::Rendezvous(event)
    }
}

impl From<quote::OutEvent> for OutEvent {
    fn from(event: quote::OutEvent) -> Self {
        OutEvent::Quote(event)
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(event_process = false)]
#[behaviour(out_event = "OutEvent")]
struct Behaviour {
    rendezvous: rendezvous::client::Behaviour,
    quote: quote::Behaviour,
    ping: ping::Behaviour,
}

#[derive(Debug)]
enum QuoteStatus {
    // We have not yet received a quote from the peer
    Pending,

    // We have received a quote from the peer. Or we have received that the peer is unreachable
    Received(Option<BidQuote>),
}

#[derive(Debug)]
enum RendezvousPointStatus {
    Dialed,  // We have initiated dialing but do not know if it succeeded or not
    Failed,  // We have initiated dialing but we failed to connect OR failed to discover
    Success, // We have connected to the rendezvous point and discovered peers
}

impl RendezvousPointStatus {
    // A rendezvous point has been "completed" if it is either successfully dialed or failed
    fn is_complete(&self) -> bool {
        matches!(
            self,
            RendezvousPointStatus::Success | RendezvousPointStatus::Failed
        )
    }
}

struct EventLoop {
    swarm: Swarm<Behaviour>,

    /// The namespace to discover peers in
    namespace: XmrBtcNamespace,

    /// List to store which rendezvous points we have either dialed / failed to dial
    rendezvous_points_status: HashMap<PeerId, RendezvousPointStatus>,

    /// The rendezvous points to dial
    rendezvous_points: Vec<(PeerId, Multiaddr)>,

    /// The addresses of peers that have been discovered and are reachable
    reachable_asb_address: HashMap<PeerId, Multiaddr>,

    /// The status of the quote for each peer
    asb_quote_status: HashMap<PeerId, QuoteStatus>,

    /// The queue of peers to dial
    /// When we discover a peer we add it is then dialed by the event loop
    to_request_quote: VecDeque<(PeerId, Vec<Multiaddr>)>,
}

impl EventLoop {
    fn new(
        swarm: Swarm<Behaviour>,
        rendezvous_points: Vec<(PeerId, Multiaddr)>,
        namespace: XmrBtcNamespace,
        dial_queue: VecDeque<(PeerId, Vec<Multiaddr>)>,
    ) -> Self {
        Self {
            swarm,
            rendezvous_points_status: Default::default(),
            rendezvous_points,
            namespace,
            reachable_asb_address: Default::default(),
            asb_quote_status: Default::default(),
            to_request_quote: dial_queue,
        }
    }

    fn is_rendezvous_point(&self, peer_id: &PeerId) -> bool {
        self.rendezvous_points
            .iter()
            .any(|(rendezvous_peer_id, _)| rendezvous_peer_id == peer_id)
    }

    fn get_rendezvous_point(&self, peer_id: &PeerId) -> Option<Multiaddr> {
        self.rendezvous_points
            .iter()
            .find(|(rendezvous_peer_id, _)| rendezvous_peer_id == peer_id)
            .map(|(_, multiaddr)| multiaddr.clone())
    }

    fn ensure_multiaddr_has_p2p_suffix(&self, peer_id: PeerId, multiaddr: Multiaddr) -> Multiaddr {
        let p2p_suffix = Protocol::P2p(peer_id);

        // If the multiaddr does not end with the p2p suffix, we add it
        if !multiaddr.ends_with(&Multiaddr::empty().with(p2p_suffix.clone())) {
            multiaddr.clone().with(p2p_suffix)
        } else {
            // If the multiaddr already ends with the p2p suffix, we return it as is
            multiaddr.clone()
        }
    }

    async fn run(mut self) -> Vec<SellerStatus> {
        // Dial all rendezvous points initially
        for (peer_id, multiaddr) in &self.rendezvous_points {
            let dial_opts = DialOpts::peer_id(*peer_id)
                .addresses(vec![multiaddr.clone()])
                .extend_addresses_through_behaviour()
                .build();

            self.rendezvous_points_status
                .insert(*peer_id, RendezvousPointStatus::Dialed);

            if let Err(e) = self.swarm.dial(dial_opts) {
                tracing::error!(%peer_id, %multiaddr, error = %e, "Failed to dial rendezvous point");
                self.rendezvous_points_status
                    .insert(*peer_id, RendezvousPointStatus::Failed);
            }
        }

        loop {
            tokio::select! {
                Some((peer_id, multiaddresses)) = async { self.to_request_quote.pop_front() } => {
                    // We do not allow an overlap of rendezvous points and quote requests
                    // because if we do we cannot distinguish between a quote request and a rendezvous point later on
                    // because we are missing state information to
                    if self.is_rendezvous_point(&peer_id) {
                        tracing::warn!(%peer_id, "Skipping quote request for rendezvous point. We do not allow an overlap of rendezvous points and quote requests");
                        continue;
                    }

                    // If we already have an entry for this peer in asb_quote_status, we skip it
                    // We probably discovered a peer at a rendezvous point which we already have an entry for locally
                    if self.asb_quote_status.contains_key(&peer_id) {
                        tracing::warn!(%peer_id, "Skipping quote request for peer. We already have an entry for this peer");
                        continue;
                    }

                    // Change the status to pending
                    self.asb_quote_status.insert(peer_id, QuoteStatus::Pending);

                    // Add all known addresses to the swarm
                    for multiaddr in multiaddresses {
                        self.swarm.add_peer_address(peer_id, multiaddr);
                    }

                    // Request a quote from the peer
                    let _request_id = self.swarm.behaviour_mut().quote.send_request(&peer_id, ());
                }
                swarm_event = self.swarm.select_next_some() => {
                    match swarm_event {
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                            if self.is_rendezvous_point(&peer_id) {
                                tracing::info!(
                                    "Connected to rendezvous point, discovering nodes in '{}' namespace ...",
                                    self.namespace
                                );

                                let namespace = rendezvous::Namespace::new(self.namespace.to_string()).expect("our namespace to be a correct string");

                                self.swarm.behaviour_mut().rendezvous.discover(
                                    Some(namespace),
                                    None,
                                    None,
                                    peer_id,
                                );
                            } else {
                                let address = endpoint.get_remote_address();
                                tracing::debug!(%peer_id, %address, "Connection established to peer");
                                self.reachable_asb_address.insert(peer_id, address.clone());
                            }
                        }
                        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                            if let Some(peer_id) = peer_id {
                                if let Some(rendezvous_point) = self.get_rendezvous_point(&peer_id) {
                                    tracing::error!(
                                        %peer_id,
                                        %rendezvous_point,
                                        "Failed to connect to rendezvous point: {}",
                                        error
                                    );

                                    // Update the status of the rendezvous point to failed
                                    self.rendezvous_points_status.insert(peer_id, RendezvousPointStatus::Failed);
                                } else {
                                    tracing::error!(
                                        %peer_id,
                                        "Failed to connect to peer: {}",
                                        error
                                    );

                                    match self.asb_quote_status.entry(peer_id) {
                                        Entry::Occupied(mut entry) => {
                                            entry.insert(QuoteStatus::Received(None));
                                        },
                                        _ => {
                                            tracing::debug!(%peer_id, %error, "Connection error with unexpected peer");
                                        }
                                    }
                                }
                            } else {
                                tracing::debug!("Failed to connect (no peer id): {}", error);
                            }
                        }
                        SwarmEvent::Behaviour(OutEvent::Rendezvous(
                                                  libp2p::rendezvous::client::Event::Discovered { registrations, rendezvous_node, .. },
                                              )) => {
                            for registration in registrations {
                                let peer = registration.record.peer_id();
                                let addresses = registration.record.addresses().into_iter().map(|addr| self.ensure_multiaddr_has_p2p_suffix(peer, addr.clone())).collect::<Vec<_>>();

                                tracing::info!(%peer, ?addresses, "Discovered peer at rendezvous point");

                                self.to_request_quote.push_back((peer, addresses));
                            }

                            // Update the status of the rendezvous point to success
                            self.rendezvous_points_status.insert(rendezvous_node, RendezvousPointStatus::Success);
                        }
                        SwarmEvent::Behaviour(OutEvent::Rendezvous(libp2p::rendezvous::client::Event::DiscoverFailed { rendezvous_node, .. })) => {
                            self.rendezvous_points_status.insert(rendezvous_node, RendezvousPointStatus::Failed);
                        }
                        SwarmEvent::Behaviour(OutEvent::Quote(quote_response)) => {
                            match quote_response {
                                request_response::Event::Message { peer, message } => {
                                    match message {
                                        request_response::Message::Response { response, .. } => {
                                            if self.asb_quote_status.insert(peer, QuoteStatus::Received(Some(response))).is_none() {
                                                tracing::error!(%peer, "Received bid quote from unexpected peer, this record will be removed!");
                                                self.asb_quote_status.remove(&peer);
                                                continue;
                                            }

                                            tracing::debug!(%peer, quote = ?response, "Received quote from peer");
                                        }
                                        request_response::Message::Request { .. } => unreachable!("we only request quotes, not respond")
                                    }
                                }
                                request_response::Event::OutboundFailure { peer, error, .. } => {
                                    if self.is_rendezvous_point(&peer) {
                                        tracing::debug!(%peer, "Outbound failure when communicating with rendezvous node: {:#}", error);
                                    } else {
                                        tracing::debug!(%peer, "Ignoring seller, because unable to request quote: {:#}", error);

                                        // Update the status of the quote to failed
                                        self.asb_quote_status.insert(peer, QuoteStatus::Received(None));
                                    }
                                }
                                request_response::Event::InboundFailure { peer, error, .. } => {
                                    if self.is_rendezvous_point(&peer) {
                                        tracing::debug!(%peer, "Inbound failure when communicating with rendezvous node: {:#}", error);

                                        // Update the status of the rendezvous point to failed
                                        self.rendezvous_points_status.insert(peer, RendezvousPointStatus::Failed);
                                    } else {
                                        tracing::debug!(%peer, "Ignoring seller, because unable to request quote: {:#}", error);

                                        // Update the status of the quote to failed
                                        self.asb_quote_status.insert(peer, QuoteStatus::Received(None));
                                    }
                                },
                                request_response::Event::ResponseSent { .. } => unreachable!()
                            }
                        }
                        _ => {}
                    }
                }
            }

            // We are finished if both of these conditions are true
            // 1. All rendezvous points have been successfully dialed or failed to dial / discover at namespace
            // 2. We don't have any pending quote requests
            // 3. We received quotes OR failed to from all peers we have requested quotes from

            // Check if all peer ids from rendezvous_points are present in rendezvous_points_status
            // Check if every entry in rendezvous_points_status is "complete"
            let all_rendezvous_points_requests_complete =
                self.rendezvous_points.iter().all(|(peer_id, _)| {
                    self.rendezvous_points_status
                        .get(peer_id)
                        .map(|status| status.is_complete())
                        .unwrap_or(false)
                });

            // Check if to_request_quote is empty
            let all_quotes_fetched = self.to_request_quote.is_empty();

            // If we have pending request to rendezvous points or quote requests, we continue
            if !all_rendezvous_points_requests_complete || !all_quotes_fetched {
                continue;
            }

            let all_quotes_fetched = self
                .asb_quote_status
                .iter()
                .map(|(peer_id, quote_status)| match quote_status {
                    QuoteStatus::Pending => Err(StillPending {}),
                    QuoteStatus::Received(Some(quote)) => {
                        let address = self
                            .reachable_asb_address
                            .get(peer_id)
                            .expect("if we got a quote we must have stored an address");

                        Ok(SellerStatus::Online(QuoteWithAddress {
                            peer_id: *peer_id,
                            multiaddr: address.clone(),
                            quote: quote.clone(),
                        }))
                    }
                    QuoteStatus::Received(None) => {
                        Ok(SellerStatus::Unreachable(UnreachableSeller {
                            peer_id: *peer_id,
                        }))
                    }
                })
                .collect::<Result<Vec<_>, _>>();

            match all_quotes_fetched {
                Ok(mut sellers) => {
                    sellers.sort();
                    break sellers;
                }
                Err(StillPending {}) => continue,
            }
        }
    }
}

#[derive(Debug)]
struct StillPending {}

impl From<ping::Event> for OutEvent {
    fn from(event: ping::Event) -> Self {
        OutEvent::Ping(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sellers_sort_with_unreachable_coming_last() {
        let mut list = vec![
            Seller {
                multiaddr: "/ip4/127.0.0.1/tcp/1234".parse().unwrap(),
                status: SellerStatus::Unreachable,
            },
            Seller {
                multiaddr: Multiaddr::empty(),
                status: SellerStatus::Unreachable,
            },
            Seller {
                multiaddr: "/ip4/127.0.0.1/tcp/5678".parse().unwrap(),
                status: SellerStatus::Online(BidQuote {
                    price: Default::default(),
                    min_quantity: Default::default(),
                    max_quantity: Default::default(),
                }),
            },
        ];

        list.sort();

        assert_eq!(
            list,
            vec![
                Seller {
                    multiaddr: "/ip4/127.0.0.1/tcp/5678".parse().unwrap(),
                    status: SellerStatus::Online(BidQuote {
                        price: Default::default(),
                        min_quantity: Default::default(),
                        max_quantity: Default::default(),
                    })
                },
                Seller {
                    multiaddr: Multiaddr::empty(),
                    status: SellerStatus::Unreachable
                },
                Seller {
                    multiaddr: "/ip4/127.0.0.1/tcp/1234".parse().unwrap(),
                    status: SellerStatus::Unreachable
                },
            ]
        )
    }
}
