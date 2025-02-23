use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};

pub trait MultiAddrExt {
    fn extract_peer_id(&self) -> Option<PeerId>;
}

impl MultiAddrExt for Multiaddr {
    fn extract_peer_id(&self) -> Option<PeerId> {
        match self.iter().last()? {
            Protocol::P2p(peer_id) => Some(peer_id),
            _ => None,
        }
    }
}
