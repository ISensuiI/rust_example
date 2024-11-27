/*
# Step 1: Generate the private key in PEM format
openssl ecparam -name secp256k1 -genkey -noout -out private_key.pem

# Step 2: Convert the PEM key to a raw hex format and save it to identity.txt
openssl ec -in private_key.pem -text -noout | grep priv -A 3 | tail -n +2 | tr -d '\n[:space:]:' > identity.txt

# Optionally, remove the PEM file
rm private_key.pem
*/
use ethers::{
  prelude::Address,
  utils::{keccak256, to_checksum},
};
use libp2p::{
  identity::{self, Keypair},
  PeerId,
};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use std::{error::Error, fs};

pub struct Identity {
  pub private_key: SecretKey,
  pub public_key: PublicKey,
  pub keypair: Keypair,
  pub peer_id: PeerId,
}

impl Identity {
  pub fn from_file(file_path: &str) -> Result<Self, Box<dyn Error>> {
    let private_key_str = fs::read_to_string(file_path)?.trim().to_string();

    if private_key_str.is_empty() {
      return Err(format!("Private key is empty in file: {}", file_path).into());
    }

    // Decode the hex string into bytes
    let private_key_bytes = hex::decode(private_key_str)?;

    // Check if the length of the private key is valid
    if private_key_bytes.len() != 32 {
      return Err("Private key must be exactly 32 bytes".into());
    }

    let secp = Secp256k1::new();

    // Parse the private key from the slice
    let private_key = SecretKey::from_slice(&private_key_bytes)
      .map_err(|_| "Invalid private key provided. Ensure it is a valid secp256k1 key.")?;

    let public_key = PublicKey::from_secret_key(&secp, &private_key);

    // Create a libp2p Keypair from the secp256k1 private key
    let secret_key = identity::secp256k1::SecretKey::try_from_bytes(private_key_bytes)?;
    let libp2p_keypair: Keypair = identity::secp256k1::Keypair::from(secret_key).into();
    let peer_id = PeerId::from(libp2p_keypair.public());

    Ok(Identity {
      private_key,
      public_key,
      keypair: libp2p_keypair,
      peer_id,
    })
  }
}

pub fn pub_key_to_eth_address(pub_key: &PublicKey) -> Result<String, Box<dyn Error>> {
  // Serialize the public key in uncompressed format (65 bytes)
  let pub_key_bytes = pub_key.serialize_uncompressed();

  // Calculate the Ethereum address by hashing the X and Y coordinates (skip the first byte)
  let hash = keccak256(&pub_key_bytes[1 ..]); // Skip the 0x04 prefix
  let address = Address::from_slice(&hash[12 ..]);

  Ok(to_checksum(&address, None))
}

pub fn compress_pub_key_to_eth_address(pub_key: &PublicKey) -> Result<String, Box<dyn Error>> {
  // Serialize the public key in compressed format (33 bytes)
  let pub_key_bytes = pub_key.serialize();

  // Calculate the Ethereum address by hashing the compressed key (only 33 bytes)
  let hash = keccak256(pub_key_bytes);
  let address = Address::from_slice(&hash[12 ..]);

  Ok(to_checksum(&address, None))
}

fn main() -> Result<(), Box<dyn Error>> {
  let identity = Identity::from_file("identity.txt")?;
  println!("secp256k1 Private Key: {:?}", identity.private_key);
  println!("secp256k1 Public Key: {:?}", identity.public_key);
  println!("libp2p secp256k1 keypair: {:?}", identity.keypair);
  println!(
    "libp2p secp256k1 keypair public: {:?}",
    identity.keypair.public()
  );
  println!("libp2p Peer ID: {}", identity.peer_id);

  let eth_address = compress_pub_key_to_eth_address(&identity.public_key)?;
  println!(
    "Compressed Ethereum Address: {}, which does nothing",
    eth_address
  );

  // Example usage of get_eth_addr_from_peer
  let eth_address = pub_key_to_eth_address(&identity.public_key)?;
  println!(
    "Ethereum Address: {}, it is the same with others",
    eth_address
  );

  Ok(())
}
