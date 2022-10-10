use futures::prelude::*;
use libp2p::swarm::{SwarmBuilder, SwarmEvent};
use libp2p::{identity, Multiaddr, PeerId};
use libp2p_perf::{build_transport, Perf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "libp2p-perf client",
    about = "The iPerf equivalent for the libp2p ecosystem."
)]
struct Opt {
    #[structopt(long)]
    server_address: Multiaddr,
}

#[async_std::main]
async fn main() {
    env_logger::init();
    let opt = Opt::from_args();

    let key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(key.public());

    let transport = build_transport(key, None).unwrap();
    let perf = Perf::default();
    let mut client = SwarmBuilder::new(transport, perf, local_peer_id)
        .executor(Box::new(|f| {
            async_std::task::spawn(f);
        }))
        .build();

    client.dial(opt.server_address).unwrap();

    loop {
        match client.next().await.expect("Infinite stream.") {
            SwarmEvent::Behaviour(e) => {
                println!("{}", e);

                // TODO: Fix hack
                //
                // Performance run timer has already been stopped. Wait for a second
                // to make sure the receiving side of the substream on the server is
                // closed before the whole connection is dropped.
                std::thread::sleep(std::time::Duration::from_secs(1));

                break;
            }
            SwarmEvent::ConnectionEstablished { .. } => {}
            e => println!("{:?}", e),
        }
    }
}
