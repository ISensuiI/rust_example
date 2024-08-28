use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::env;
use tokio;

#[derive(Serialize, Deserialize, Debug)]
struct User {
  id: i32,
  name: String,
  email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Load environment variables
  dotenv::dotenv().ok();

  let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");

  // Create Redis client and connection
  let redis_client = redis::Client::open(redis_url)?;
  let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;

  let new_user = User {
    id: 1,
    name: "John Doe".to_string(),
    email: "john.doe@example.com".to_string(),
  };

  let data: Vec<u8> = bincode::serialize(&new_user).unwrap();

  // Set a key with a value and an expiration time of 60 seconds
  let _: () = redis_conn.set_ex("mykey", data, 60).await?;

  println!("Key 'mykey' set with value and TTL of 60 seconds.");

  // Retrieve the serialized data back from Redis
  let result: Option<Vec<u8>> = redis_conn.get("mykey").await?;

  // Check if the data was found and deserialize it
  if let Some(data) = result {
    let retrieved_user: User = bincode::deserialize(&data).unwrap();
    println!("Retrieved user from Redis: {:?}", retrieved_user);
  } else {
    println!("Key 'mykey' not found in Redis.");
  }

  Ok(())
}
