use serde_json::{json, Value};
use std::error::Error;

const BASE_URL: &str = "http://192.168.15.222:8011/stats";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let client = reqwest::Client::new();

  let url1 = format!("{}/statistic-queries", BASE_URL);

  // First request
  let json1 = json!({
      "deployment": ["QmbReTnhCweQHmbXxgffkDqkkedo7ojjsUWTKopP1auuTp", "QmTfhYrb3wusYS715KvHfaL56R8M1SrM8vwhuLyYVehfKB"],
      "start_date": "2024-09-01",
  });

  let res = client
    .post(&url1)
    .json(&json1)
    .send()
    .await?
    .json::<Value>()
    .await?;

  println!("1: {:#?}", res);

  // Second request
  let json2 = json!({
      "indexer": ["0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64", "0xbf3a286a477967ebd850cee2dbdbfa622222"],
      "start_date": "2024-09-01",
  });

  let res = client.post(&url1).json(&json2).send().await?.text().await?;

  println!("2: {:#?}", res);

  // Third request
  let json3 = json!({
      "indexer": ["0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64", "0xbf3a286a477967ebd850cee2dbdbfa6e5222224"],
      "deployment": ["QmbReTnhCweQHmbXxgffkDqkkedo7ojjsUWTKopP1auuTp", "QmTfhYrb3wusYS715KvHfaL56R8M1SrM8vwhuLyYVehfKB"],
      "start_date": "2024-09-01",
  });

  let res = client.post(&url1).json(&json3).send().await?.text().await?;

  println!("3: {:#?}", res);

  // Fourth request
  let json4 = json!({
      "deployment": ["QmbReTnhCweQHmbXxgffkDqkkedo7ojjsUWTKopP1auuTp", "QmTfhYrb3wusYS715KvHfaL56R8M1SrM8vwhuLyYVehfKB"],
      "start_date": "2024-09-01",
  });

  let url2 = format!("{}/deployment-price-count", BASE_URL);
  let res = client.post(&url2).json(&json4).send().await?.text().await?;

  println!("4 {:#?}", res);

  let url3 = format!(
    "{}/user_aggregation/0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64",
    BASE_URL
  );

  // Fifth request
  let res = client.get(&url3).send().await?.text().await?;

  println!("5 {:#?}", res);

  // Sixth request
  let json4 = json!({
      "user_list": ["0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64", "0xa499b9c52547db14d13216dcd73da0d4d43ba66e", "0x5e15ce35a3821e15d36988d9e0dd181c7c371a07"],
      "start": "2024-09-01",
      "end": "2024-09-02"
  });

  let url4 = format!("{}/multi_user_aggregation", BASE_URL);

  let res = client
    .post(&url4)
    .json(&json4)
    .send()
    .await?
    .json::<Value>()
    .await?;

  println!("6 {:#?}", res);

  // Seventh request
  let json4 = json!({
      "user_list": ["0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64", "0xa499b9c52547db14d13216dcd73da0d4d43ba66e", "0x5e15ce35a3821e15d36988d9e0dd181c7c371a07"],
      "start": "2024-09-13"
  });

  let res = client
    .post(&url4)
    .json(&json4)
    .send()
    .await?
    .json::<Value>()
    .await?;

  println!("7 {:#?}", res);

  let url5 = format!(
    "{}/statistic-indexer-channel?indexer=0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64&\
     deployment=QmNevi2wSvFzigFXrQdPTQFQxVEbpfmZ2uLX1HKxYj5dY8",
    BASE_URL
  );
  // Eighth request
  let res = client.get(&url5).send().await?.text().await?;

  println!("8 {:#?}", res);

  Ok(())
}
