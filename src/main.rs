use std::{collections::HashMap, sync::{Arc, Mutex}, thread::{self, sleep}, time::{Duration, Instant}};

use config::Config;
use mail_send::{mail_builder::MessageBuilder, SmtpClientBuilder};
use tokio::net::UdpSocket;
use uuid::Uuid;

const PORT: u16 = 28915;

#[tokio::main]
async fn main() {
	let config = Config::builder()
		.add_source(config::File::with_name("config"))
		.add_source(config::Environment::with_prefix("HEARTBEAT"))
		.build()
		.unwrap();

	let servers = config.get_array("servers").unwrap();
	let mut server_heartbeats: HashMap<Uuid, Instant> = HashMap::new();
	let mut server_names: HashMap<Uuid, String> = HashMap::new();

	for server in servers {
		let server = server.into_table().unwrap();
		let name = server.get("name").unwrap();
		let uuid = server.get("uuid").unwrap();
		println!("Server: {} ({})", name, uuid);
		server_heartbeats.insert(Uuid::parse_str(uuid.to_string().as_str()).unwrap(), Instant::now());
		server_names.insert(Uuid::parse_str(uuid.to_string().as_str()).unwrap(), name.to_string());
	}

	let server_heartbeats = Arc::new(Mutex::new(server_heartbeats));
	let down_servers: Arc<Mutex<Vec<Uuid>>> = Arc::new(Mutex::new(Vec::new()));

	{
		let server_heartbeats = Arc::clone(&server_heartbeats);
		thread::spawn(move || {
			tokio::runtime::Runtime::new().unwrap().block_on(udp_server(server_heartbeats));
		});
	}

	check_thread(config, server_names, server_heartbeats, down_servers).await;
}

async fn udp_server(server_heartbeats: Arc<Mutex<std::collections::HashMap<Uuid, std::time::Instant>>>) {
	let socket = UdpSocket::bind(format!("0.0.0.0:{}", PORT)).await.expect("Failed to bind socket");
	let mut buf = [0; 36];

	loop {
		let (len, addr) = socket.recv_from(&mut buf).await.expect("Failed to receive data");
		println!("Received {} bytes from {}", len, addr);

		let data = String::from_utf8_lossy(&buf[..len]);
		let data = data.trim();
		let uuid = Uuid::parse_str(&data);
		if uuid.is_err() {
			println!("Received invalid UUID from {}: {}", addr, data);
			continue;
		}

		let uuid = uuid.unwrap();
		let mut server_heartbeats = server_heartbeats.lock().unwrap();
		if let Some(heartbeat) = server_heartbeats.get_mut(&uuid) {
			*heartbeat = std::time::Instant::now();
			println!("Received heartbeat from server: {}", uuid);
		} else {
			println!("Received heartbeat from unknown server: {}", uuid);
		}
	}
}

async fn check_thread(config: Config, server_names: std::collections::HashMap<Uuid, String>, server_heartbeats: Arc<Mutex<std::collections::HashMap<Uuid, std::time::Instant>>>, down_servers: Arc<Mutex<Vec<Uuid>>>) {
	loop {
		let server_heartbeats = server_heartbeats.lock().unwrap();
		let mut down_servers = down_servers.lock().unwrap();
		let now = std::time::Instant::now();
		for (uuid, heartbeat) in server_heartbeats.iter() {
			let name = server_names.get(uuid).unwrap();
			if now.duration_since(*heartbeat) > Duration::from_secs(10) {
				// Server is down!
				if !down_servers.contains(uuid) {
					down_servers.push(*uuid);
					notify_down(&config, uuid, name).await;
				}
			} else {
				// Server is up!
				if let Some(index) = down_servers.iter().position(|down_uuid| down_uuid == uuid) {
					down_servers.remove(index);
					notify_up(&config, uuid, name).await;
				}
			}
		}
		drop(server_heartbeats);

		sleep(Duration::from_secs(5));
	}
}

async fn notify_down(config: &Config, uuid: &Uuid, name: &String) {
	println!("Server {} has not sent a heartbeat in 10 seconds!", uuid);
	let message = MessageBuilder::new()
		.from((config.get_string("smtp.fromName").unwrap(), config.get_string("smtp.fromEmail").unwrap()))
		.to(vec![
			(config.get_string("smtp.toName").unwrap(), config.get_string("smtp.toEmail").unwrap())
		])
		.subject(
			config.get_string("smtp.down_subject").unwrap()
			.replace("%UUID%", uuid.to_string().as_str())
			.replace("%NAME%", name)
		)
		.text_body(
			config.get_string("smtp.down_body").unwrap()
			.replace("%UUID%", uuid.to_string().as_str())
			.replace("%NAME%", name)
		);
	
	SmtpClientBuilder::new(config.get_string("smtp.hostname").unwrap(), config.get_int("smtp.port").unwrap() as u16)
			.implicit_tls(false)
			.credentials((config.get_string("smtp.username").unwrap(), config.get_string("smtp.password").unwrap()))
			.connect()
			.await
			.unwrap()
			.send(message)
			.await
			.unwrap();
}

async fn notify_up(config: &Config, uuid: &Uuid, name: &String) {
	println!("Server {} is back online!", uuid);
	let message = MessageBuilder::new()
		.from((config.get_string("smtp.fromName").unwrap(), config.get_string("smtp.fromEmail").unwrap()))
		.to(vec![
			(config.get_string("smtp.toName").unwrap(), config.get_string("smtp.toEmail").unwrap())
		])
		.subject(
			config.get_string("smtp.up_subject").unwrap()
			.replace("%UUID%", uuid.to_string().as_str())
			.replace("%NAME%", name)
		)
		.text_body(
			config.get_string("smtp.up_body").unwrap()
			.replace("%UUID%", uuid.to_string().as_str())
			.replace("%NAME%", name)
		);
	
	SmtpClientBuilder::new(config.get_string("smtp.hostname").unwrap(), config.get_int("smtp.port").unwrap() as u16)
			.implicit_tls(false)
			.credentials((config.get_string("smtp.username").unwrap(), config.get_string("smtp.password").unwrap()))
			.connect()
			.await
			.unwrap()
			.send(message)
			.await
			.unwrap();
}
