use std::net::SocketAddr;
use std::time::Duration;
use netstack::{
    server::{
        Configuration,
        Server,
        Event,
    },
    transport::UdpTransport,
    time::Clock,
    security::{
        Secret,
        ConnectionToken,
    },
    packets::OutgoingPacket,
};
use std::io::Write;
use std::thread;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{sync_channel, SyncSender};
use simple_server::Server as WebServer;
use std::sync::Arc;

fn generate_secret() -> Secret {
    use rand::Rng;
    let random_bytes = rand::thread_rng().gen::<[u8; 32]>();

    Secret::from_bytes(random_bytes)
}

fn generate_connection_token() -> ConnectionToken {
    use rand::Rng;
    let random_bytes = rand::thread_rng().gen::<[u8; 32]>();

    ConnectionToken::from_bytes(random_bytes)
}

#[derive(Serialize, Deserialize)]
pub struct ConnectionInfo {
    token: String,
    secret: String,
}

fn run_webserver(sender: Arc<SyncSender<(ConnectionToken, Secret)>>) {
    use base58::ToBase58;

    thread::spawn(move || {
        let webserver = WebServer::new(move |_, mut response| {
            let token = generate_connection_token();
            let secret = generate_secret();

            let token_string = token.get_bytes().to_base58();
            let secret_string = secret.get_bytes().to_base58();

            let info = ConnectionInfo {
                token: token_string,
                secret: secret_string,
            };
            let send_info = serde_json::to_string(&info).unwrap();
            sender.send((token, secret)).unwrap();

            Ok(response.body(send_info.as_bytes().to_vec())?)
        });
    
        webserver.listen("127.0.0.1", "8000");
    });
}

fn main() {
    let local_address: SocketAddr = "127.0.0.1:9000".parse().unwrap();
    let transport = UdpTransport::new(local_address).unwrap();
    let mut clock = Clock::new(Duration::from_millis(16));

    let config = Configuration {
        max_connections: 64,
        timeout: 60,
        reserved_timeout: 120,
    };

    let mut server = Server::new(config, Box::new(transport));

    let (sender, receiver) = sync_channel(2);
    run_webserver(Arc::new(sender));

    loop {
        match receiver.try_recv() {
            Ok((token, secret)) => {
                println!("reserve connection");
                server.reserve(secret);
            },
            _ => {},
        }

        if clock.update() {
            let events = server.update();
    
            for event in events {
                match event {
                    Event::Connected { connection } => {
                        println!("{} connected", connection);
                    },
                    Event::Disconnected { connection } => {
                        println!("{} disconnected", connection);
                    },
                    Event::Message{ connection, payload } => {
                        println!("Message from {}", connection);

                        let mut packet = OutgoingPacket::new();
                        packet.write(payload.get_buffer()).unwrap();

                        server.send(packet, connection).unwrap();
                    }
                }
            }
        }
    }
}