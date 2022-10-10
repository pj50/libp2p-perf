mod behaviour;
mod handler;
mod protocol;

pub use behaviour::{Perf, PerfEvent};

use libp2p::{
    core::{
        self,
        muxing::StreamMuxerBox,
        transport::Transport,
        Multiaddr,
    },
    identity,
    quic, PeerId,
};

#[derive(Debug)]
pub enum TransportSecurity {
    Noise,
    Plaintext,
    All,
}

impl std::str::FromStr for TransportSecurity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "noise" => Ok(Self::Noise),
            "plaintext" => Ok(Self::Plaintext),
            "all" => Ok(Self::All),
            _ => Err("Expected one of 'noise', 'plaintext' or 'all'.".to_string()),
        }
    }
}

impl std::fmt::Display for TransportSecurity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub fn build_transport(
    keypair: identity::Keypair,
    quic_addr: Option<Multiaddr>,
) -> std::io::Result<core::transport::Boxed<(PeerId, StreamMuxerBox)>> {
    // v3
    // let mut quic_transport =
    //     quic::GenTransport::<quic::async_std::Provider>::new(quic::Config::new(&keypair).unwrap());

    // v4
    let mut quic_transport = quic::QuicTransport::new(quic::Config::new(&keypair).unwrap());

    quic_transport
        .listen_on(quic_addr.unwrap_or("/ip4/0.0.0.0/udp/0/quic".parse().unwrap()))
        .unwrap();

    Ok(Transport::map(quic_transport, |(peer, muxer), _| {
        (peer, StreamMuxerBox::new(muxer))
    })
    .boxed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::LocalPool;

    use futures::prelude::*;
    use futures::stream::StreamExt;
    use futures::task::Spawn;
    use libp2p::core::multiaddr::{Multiaddr, Protocol};
    use libp2p::swarm::{Swarm, SwarmEvent};
    use rand::random;

    use std::time::Duration;

    #[test]
    fn it_works() {
        let mut pool = LocalPool::new();
        let _ = env_logger::try_init();

        let mut sender = {
            let key = identity::Keypair::generate_ed25519();
            let local_peer_id = PeerId::from(key.public());

            let transport = build_transport(true, key, TransportSecurity::Plaintext).unwrap();
            let perf = Perf::default();
            Swarm::new(transport, perf, local_peer_id)
        };

        let mut receiver = {
            let key = identity::Keypair::generate_ed25519();
            let local_peer_id = PeerId::from(key.public());

            let transport = build_transport(true, key, TransportSecurity::Plaintext).unwrap();
            let perf = Perf::default();
            Swarm::new(transport, perf, local_peer_id)
        };
        let receiver_address: Multiaddr = Protocol::Memory(random::<u64>()).into();

        // Wait for receiver to bind to listen address.
        pool.run_until(async {
            let id = receiver.listen_on(receiver_address.clone()).unwrap();
            match receiver.next().await.unwrap() {
                SwarmEvent::NewListenAddr { listener_id, .. } if listener_id == id => {}
                _ => panic!("Unexpected event."),
            }
        });

        pool.spawner()
            .spawn_obj(
                async move {
                    loop {
                        receiver.next().await;
                    }
                }
                .boxed()
                .into(),
            )
            .unwrap();

        sender.dial(receiver_address).unwrap();

        pool.run_until(async move {
            loop {
                match sender.next().await.unwrap() {
                    SwarmEvent::Behaviour(PerfEvent::PerfRunDone(duration, _transfered)) => {
                        if duration < Duration::from_secs(10) {
                            panic!("Expected test to run at least 10 seconds.")
                        }

                        if duration > Duration::from_secs(11) {
                            panic!("Expected test to run roughly 10 seconds.")
                        }

                        break;
                    }
                    _ => {}
                }
            }
        });
    }
}
