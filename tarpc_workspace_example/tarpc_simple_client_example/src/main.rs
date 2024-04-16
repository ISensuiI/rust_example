use std::net::{IpAddr, Ipv4Addr};
use tarpc::{client, context, tokio_serde::formats::Json};
use tokio::time::{sleep, Duration};

#[tarpc::service]
pub trait HelloWorld {
  async fn hello(name: String) -> String;
}

#[derive(Clone)]
pub struct HelloWorldImpl;

impl HelloWorld for HelloWorldImpl {
  async fn hello(self, _: context::Context, name: String) -> String {
    format!("Hello, {}!", name)
  }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let server_addr = (IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

  let mut transport = tarpc::serde_transport::tcp::connect(&server_addr, Json::default);
  transport.config_mut().max_frame_length(usize::MAX);

  // WorldClient is generated by the service attribute. It has a constructor `new` that takes a
  // config and any Transport as input.
  let client = HelloWorldClient::new(client::Config::default(), transport.await?).spawn();

  let hello = async move {
    // Send the request twice, just to be safe! ;)
    tokio::select! {
        hello1 = client.hello(context::current(), format!("hello")) => { hello1 }
        hello2 = client.hello(context::current(), format!("world")) => { hello2 }
    }
  }
  .await;

  match hello {
    Ok(hello) => println!("{hello:?}"),
    Err(e) => println!("{:?}", anyhow::Error::from(e)),
  }

  // Let the background span processor finish.
  sleep(Duration::from_micros(1)).await;

  Ok(())
}
