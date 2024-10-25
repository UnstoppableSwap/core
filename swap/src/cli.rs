pub mod api;
mod behaviour;
pub mod cancel_and_refund;
pub mod command;
mod event_loop;
mod list_sellers;
pub mod transport;
pub mod watcher;

pub use behaviour::{Behaviour, OutEvent};
pub use cancel_and_refund::{cancel, cancel_and_refund, refund};
pub use event_loop::{EventLoop, EventLoopHandle};
pub use list_sellers::{list_sellers, Seller, Status as SellerStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asb;
    use crate::asb::rendezvous::RendezvousNode;
    use crate::cli::list_sellers::{Seller, Status};
    use crate::network::quote;
    use crate::network::quote::BidQuote;
    use crate::network::rendezvous::XmrBtcNamespace;
    use crate::network::test::{new_swarm, SwarmExt};
    use futures::StreamExt;
    use libp2p::multiaddr::Protocol;
    use libp2p::{identity, rendezvous, Multiaddr, PeerId, request_response};
    use std::collections::HashSet;
    use std::iter::FromIterator;
    use std::time::Duration;

    #[tokio::test]
    async fn list_sellers_should_report_all_registered_asbs_with_a_quote() {
        let namespace = XmrBtcNamespace::Mainnet;
        let (rendezvous_address, rendezvous_peer_id) = setup_rendezvous_point().await;
        let expected_seller_1 = setup_asb(rendezvous_peer_id, &rendezvous_address, namespace).await;
        let expected_seller_2 = setup_asb(rendezvous_peer_id, &rendezvous_address, namespace).await;

        let list_sellers = list_sellers(
            rendezvous_peer_id,
            rendezvous_address,
            namespace,
            0,
            identity::Keypair::generate_ed25519(),
        );
        let sellers = tokio::time::timeout(Duration::from_secs(15), list_sellers)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            HashSet::<Seller>::from_iter(sellers),
            HashSet::<Seller>::from_iter([expected_seller_1, expected_seller_2])
        )
    }

    async fn setup_rendezvous_point() -> (Multiaddr, PeerId) {
        let mut rendezvous_node = new_swarm(|_| RendezvousPointBehaviour::default());
        let rendezvous_address = rendezvous_node.listen_on_tcp_localhost().await;
        let rendezvous_peer_id = *rendezvous_node.local_peer_id();

        tokio::spawn(async move {
            loop {
                rendezvous_node.next().await;
            }
        });

        (rendezvous_address, rendezvous_peer_id)
    }

    async fn setup_asb(
        rendezvous_peer_id: PeerId,
        rendezvous_address: &Multiaddr,
        namespace: XmrBtcNamespace,
    ) -> Seller {
        let static_quote = BidQuote {
            price: bitcoin::Amount::from_sat(1337),
            min_quantity: bitcoin::Amount::from_sat(42),
            max_quantity: bitcoin::Amount::from_sat(9001),
        };

        let mut asb = new_swarm(|identity| {
            let rendezvous_node =
                RendezvousNode::new(rendezvous_address, rendezvous_peer_id, namespace, None);
            let rendezvous = asb::rendezvous::Behaviour::new(identity, vec![rendezvous_node]);

            StaticQuoteAsbBehaviour {
                rendezvous,
                quote: quote::asb(),
                // static_quote,
                // registered: false,
            }
        });

        let asb_address = asb.listen_on_tcp_localhost().await;
        asb.add_external_address(asb_address.clone());

        let asb_peer_id = *asb.local_peer_id();

        // avoid race condition where `list_sellers` tries to discover before we are
        // registered block this function until we are registered
        while !asb.behaviour().registered {
            asb.next().await;
        }

        tokio::spawn(async move {
            loop {
                asb.next().await;
            }
        });

        Seller {
            multiaddr: asb_address.with(Protocol::P2p(asb_peer_id.into())),
            status: Status::Online(static_quote),
        }
    }

    #[derive(libp2p::swarm::NetworkBehaviour)]
    struct StaticQuoteAsbBehaviour {
        rendezvous: asb::rendezvous::Behaviour,
        quote: quote::Behaviour,

        // TODO(Libp2p Migration): Support for this macro attribute has been removed: https://github.com/libp2p/rust-libp2p/pull/2842
        // We need to find a way to ignore this field, put it somewhere else
        #[behaviour(ignore)]
        static_quote: BidQuote,
        #[behaviour(ignore)]
        registered: bool,
    }
    impl NetworkBehaviourEventProcess<rendezvous::client::Event> for StaticQuoteAsbBehaviour {
        fn inject_event(&mut self, event: rendezvous::client::Event) {
            if let rendezvous::client::Event::Registered { .. } = event {
                self.registered = true;
            }
        }
    }

    impl NetworkBehaviourEventProcess<libp2p::ping::Event> for StaticQuoteAsbBehaviour {
        fn inject_event(&mut self, _: libp2p::ping::Event) {}
    }

    impl NetworkBehaviourEventProcess<quote::OutEvent> for StaticQuoteAsbBehaviour {
        fn inject_event(&mut self, event: quote::OutEvent) {
            if let request_response::Event::Message {
                message: quote::Message::Request { channel, .. },
                ..
            } = event
            {
                self.quote
                    .send_response(channel, self.static_quote)
                    .unwrap();
            }
        }
    }

    #[derive(libp2p::swarm::NetworkBehaviour)]
    struct RendezvousPointBehaviour {
        rendezvous: rendezvous::server::Behaviour,
    }

    impl NetworkBehaviourEventProcess<rendezvous::server::Event> for RendezvousPointBehaviour {
        fn inject_event(&mut self, _: rendezvous::server::Event) {}
    }

    impl Default for RendezvousPointBehaviour {
        fn default() -> Self {
            RendezvousPointBehaviour {
                rendezvous: rendezvous::server::Behaviour::new(
                    rendezvous::server::Config::default(),
                ),
            }
        }
    }
}
