use crate::cli;
use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use futures::future::FutureExt;
use libp2p::core::Multiaddr;
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::{NetworkBehaviour, ToSwarm};
use libp2p::PeerId;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Instant, Sleep};
use void::Void;

pub enum OutEvent {
    AllAttemptsExhausted { peer: PeerId },
}

/// A [`NetworkBehaviour`] that tracks whether we are connected to the given
/// peer and attempts to re-establish a connection with an exponential backoff
/// if we lose the connection.
pub struct Behaviour {
    /// The peer we are interested in.
    peer: PeerId,
    /// If present, tracks for how long we need to sleep until we dial again.
    sleep: Option<Pin<Box<Sleep>>>,
    /// Tracks the current backoff state.
    backoff: ExponentialBackoff,
}

impl Behaviour {
    pub fn new(peer: PeerId, interval: Duration) -> Self {
        Self {
            peer,
            sleep: None,
            backoff: ExponentialBackoff {
                initial_interval: interval,
                current_interval: interval,
                // Never give up dialing
                // TOOD: Libp2p Upgrade: Is this the right approach?
                max_elapsed_time: None,
                ..ExponentialBackoff::default()
            },
        }
    }

    pub fn until_next_redial(&self) -> Option<Duration> {
        let until_next_redial = self
            .sleep
            .as_ref()?
            .deadline()
            .checked_duration_since(Instant::now())?;

        Some(until_next_redial)
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = libp2p::swarm::dummy::ConnectionHandler;
    type ToSwarm = OutEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        if peer == self.peer {
            self.sleep = None; // Cancel any active re-dialling
        }
        Ok(Self::ConnectionHandler {})
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        if peer == self.peer {
            self.sleep = None; // Cancel any active re-dialling
        }
        Ok(Self::ConnectionHandler {})
    }

    fn on_swarm_event(&mut self, event: libp2p::swarm::FromSwarm<'_>) {
        // TODO(Libp2p Migration): Maybe we also need to match for FromSwarm::DialFailure here?

        if let libp2p::swarm::FromSwarm::ConnectionClosed(closed) = event {
            if closed.peer_id == self.peer {
                // Lost connection, trigger re-dialling with exponential backoff
                self.backoff.reset();
                self.sleep = Some(Box::pin(tokio::time::sleep(self.backoff.initial_interval)));
            }
        }
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> std::task::Poll<ToSwarm<Self::ToSwarm, Void>> {
        let sleep = match self.sleep.as_mut() {
            None => return Poll::Pending, // early exit if we shouldn't be re-dialling
            Some(future) => future,
        };

        futures::ready!(sleep.poll_unpin(cx));

        let next_dial_in = match self.backoff.next_backoff() {
            Some(next_dial_in) => next_dial_in,
            None => {
                return Poll::Ready(ToSwarm::GenerateEvent(OutEvent::AllAttemptsExhausted {
                    peer: self.peer,
                }));
            }
        };

        self.sleep = Some(Box::pin(tokio::time::sleep(next_dial_in)));

        Poll::Ready(ToSwarm::Dial {
            opts: DialOpts::peer_id(self.peer)
                .condition(PeerCondition::Disconnected)
                .build(),
        })
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: libp2p::swarm::ConnectionId,
        _event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        // the dummy connection handler does not produce any events
        // therefore we do not need to handle any events here
        // TODO(Libp2p Migration): Is this correct?
        unreachable!("The re-dial dummy connection handler does not produce any events");
    }
}

impl From<OutEvent> for cli::OutEvent {
    fn from(event: OutEvent) -> Self {
        match event {
            OutEvent::AllAttemptsExhausted { peer } => {
                cli::OutEvent::AllRedialAttemptsExhausted { peer }
            }
        }
    }
}
