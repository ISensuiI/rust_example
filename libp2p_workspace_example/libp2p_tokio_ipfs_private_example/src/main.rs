use either::Either;
use futures::prelude::*;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport, transport::upgrade::Version},
    gossipsub, identify, identity,
    multiaddr::Protocol,
    noise, ping,
    pnet::{PnetConfig, PreSharedKey},
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use std::{env, error::Error, fs, path::Path, str::FromStr, time::Duration};
use tokio::io::{self, AsyncBufReadExt};

/// Builds the transport that serves as a common ground for all connections.
pub fn build_transport(
    key_pair: identity::Keypair,
    psk: Option<PreSharedKey>,
) -> transport::Boxed<(PeerId, StreamMuxerBox)> {
    let noise_config = noise::Config::new(&key_pair).unwrap();
    let yamux_config = yamux::Config::default();

    let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
    let maybe_encrypted = match psk {
        Some(psk) => Either::Left(
            base_transport.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket)),
        ),
        None => Either::Right(base_transport),
    };
    maybe_encrypted
        .upgrade(Version::V1Lazy)
        .authenticate(noise_config)
        .multiplex(yamux_config)
        .timeout(Duration::from_secs(20))
        .boxed()
}

/// Get the current ipfs repo path, either from the IPFS_PATH environment variable or
/// from the default $HOME/.ipfs
fn get_ipfs_path() -> Box<Path> {
    env::var("IPFS_PATH")
        .map(|ipfs_path| Path::new(&ipfs_path).into())
        .unwrap_or_else(|_| {
            env::var("HOME")
                .map(|home| Path::new(&home).join(".ipfs"))
                .expect("could not determine home directory")
                .into()
        })
}

/// Read the pre shared key file from the given ipfs directory
fn get_psk(path: &Path) -> std::io::Result<Option<String>> {
    let swarm_key_file = path.join("swarm.key");
    match fs::read_to_string(swarm_key_file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// for a multiaddr that ends with a peer id, this strips this suffix. Rust-libp2p
/// only supports dialing to an address without providing the peer id.
fn strip_peer_id(addr: &mut Multiaddr) {
    let last = addr.pop();
    match last {
        Some(Protocol::P2p(peer_id)) => {
            let mut addr = Multiaddr::empty();
            addr.push(Protocol::P2p(peer_id));
            println!("removing peer id {addr} so this address can be dialed by rust-libp2p");
        }
        Some(other) => addr.push(other),
        _ => {}
    }
}

/// parse a legacy multiaddr (replace ipfs with p2p), and strip the peer id
/// so it can be dialed by rust-libp2p
fn parse_legacy_multiaddr(text: &str) -> Result<Multiaddr, Box<dyn Error>> {
    let sanitized = text
        .split('/')
        .map(|part| if part == "ipfs" { "p2p" } else { part })
        .collect::<Vec<_>>()
        .join("/");
    let mut res = Multiaddr::from_str(&sanitized)?;
    strip_peer_id(&mut res);
    Ok(res)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let ipfs_path = get_ipfs_path();
    println!("using IPFS_PATH {ipfs_path:?}");
    let psk: Option<PreSharedKey> = get_psk(&ipfs_path)?
        .map(|text| PreSharedKey::from_str(&text))
        .transpose()?;

    // Create a random PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("using random peer id: {local_peer_id:?}");
    if let Some(psk) = psk {
        println!("using swarm key with fingerprint: {}", psk.fingerprint());
    }

    // Set up a an encrypted DNS-enabled TCP Transport over and Yamux protocol
    let transport = build_transport(local_key.clone(), psk);

    // Create a Gosspipsub topic
    let gossipsub_topic = gossipsub::IdentTopic::new("chat");

    // We create a custom network behaviour that combines gossipsub, ping and identify.
    #[derive(NetworkBehaviour)]
    #[behaviour(to_swarm = "MyBehaviourEvent")]
    struct MyBehaviour {
        gossipsub: gossipsub::Behaviour,
        identify: identify::Behaviour,
        ping: ping::Behaviour,
    }

    enum MyBehaviourEvent {
        Gossipsub(gossipsub::Event),
        Identify(identify::Event),
        Ping(ping::Event),
    }

    impl From<gossipsub::Event> for MyBehaviourEvent {
        fn from(event: gossipsub::Event) -> Self {
            MyBehaviourEvent::Gossipsub(event)
        }
    }

    impl From<identify::Event> for MyBehaviourEvent {
        fn from(event: identify::Event) -> Self {
            MyBehaviourEvent::Identify(event)
        }
    }

    impl From<ping::Event> for MyBehaviourEvent {
        fn from(event: ping::Event) -> Self {
            MyBehaviourEvent::Ping(event)
        }
    }

    // Create a Swarm to manage peers and events
    let mut swarm = {
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .max_transmit_size(262144)
            .build()
            .expect("valid config");
        let mut behaviour = MyBehaviour {
            gossipsub: gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(local_key.clone()),
                gossipsub_config,
            )
            .expect("Valid configuration"),
            identify: identify::Behaviour::new(identify::Config::new(
                "/ipfs/0.1.0".into(),
                local_key.public(),
            )),
            ping: ping::Behaviour::new(ping::Config::new()),
        };

        println!("Subscribing to {gossipsub_topic:?}");
        behaviour.gossipsub.subscribe(&gossipsub_topic).unwrap();
        SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build()
    };

    // Reach out to other nodes if specified
    for to_dial in std::env::args().skip(1) {
        let addr: Multiaddr = parse_legacy_multiaddr(&to_dial)?;
        swarm.dial(addr)?;
        println!("Dialed {to_dial:?}")
    }

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Kick it off
    loop {
        tokio::select! {
            line = stdin.next_line() => {
                if let Err(e) = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(gossipsub_topic.clone(), line.expect("Stdin not to close").expect("Stdin not to close").as_bytes())
                {
                    println!("Publish error: {e:?}");
                }
            },
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {address:?}");
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Identify(event)) => {
                        println!("identify: {event:?}");
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: peer_id,
                        message_id: id,
                        message,
                    })) => {
                        println!(
                            "Got message: {} with id: {} from peer: {:?}",
                            String::from_utf8_lossy(&message.data),
                            id,
                            peer_id
                        )
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Ping(event)) => {
                        match event {
                            ping::Event {
                                peer,
                                result: Result::Ok(rtt),
                                ..
                            } => {
                                println!(
                                    "ping: rtt to {} is {} ms",
                                    peer.to_base58(),
                                    rtt.as_millis()
                                );
                            }
                            ping::Event {
                                peer,
                                result: Result::Err(ping::Failure::Timeout),
                                ..
                            } => {
                                println!("ping: timeout to {}", peer.to_base58());
                            }
                            ping::Event {
                                peer,
                                result: Result::Err(ping::Failure::Unsupported),
                                ..
                            } => {
                                println!("ping: {} does not support ping protocol", peer.to_base58());
                            }
                            ping::Event {
                                peer,
                                result: Result::Err(ping::Failure::Other { error }),
                                ..
                            } => {
                                println!("ping: ping::Failure with {}: {error}", peer.to_base58());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
