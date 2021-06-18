use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use udp_chat::{ChatNotifyPacket, Packets, MAX_PACKET_SIZE};

struct Users {
    users: HashMap<SocketAddr, User>,
}

impl Users {
    fn new() -> Self {
        Self {
            users: HashMap::new(),
        }
    }

    fn add_user(&mut self, addr: SocketAddr, name: String) {
        self.users.insert(addr, User::new(name));
    }

    fn remove_user(&mut self, addr: SocketAddr) {
        self.users.remove(&addr);
    }

    fn get_name(&self, addr: SocketAddr) -> Option<String> {
        self.users.get(&addr).map(|user| user.name.clone())
    }

    async fn tick(&mut self, socket: &UdpSocket) {
        // remove connection expired user
        {
            let now = std::time::Instant::now();
            self.users.retain(|_, v| !v.is_expired(now));
        }

        // send ping to users
        {
            let packet = Packets::Ping;
            let packet_json = serde_json::to_string(&packet).unwrap();
            let packet_buf = packet_json.as_bytes();
            for (user_addr, _) in &self.users {
                if let Err(err) = socket.send_to(packet_buf, user_addr).await {
                    eprintln!("fail to send ping {:?}", err);
                }
            }
        }
    }

    fn ping_received(&mut self, addr: SocketAddr) {
        if let Some(user) = self.users.get_mut(&addr) {
            user.update_ping();
        }
    }

    async fn send(&self, socket: &UdpSocket, name: &str, contents: &str) {
        let packet = Packets::ChatNotify(ChatNotifyPacket {
            name: name.to_string(),
            contents: contents.to_string(),
        });

        let packet_json = serde_json::to_string(&packet).unwrap();
        let packet_buf = packet_json.as_bytes();
        if packet_buf.len() >= MAX_PACKET_SIZE {
            eprintln!("packet size overflow. {}", packet_buf.len());
        } else {
            for (user_addr, _) in &self.users {
                match socket.send_to(packet_buf, user_addr).await {
                    Ok(size) => {
                        eprintln!("send to {:?}, size: {}", user_addr, size);
                    }
                    Err(err) => {
                        eprintln!("fail to send chat notify. {:?}", err);
                    }
                }
            }
        }
    }
}

struct User {
    name: String,
    last_ping: std::time::Instant,
}

impl User {
    const EXPIRED: std::time::Duration = std::time::Duration::from_secs(5);

    fn new(name: String) -> Self {
        Self {
            name,
            last_ping: std::time::Instant::now(),
        }
    }

    fn update_ping(&mut self) {
        self.last_ping = std::time::Instant::now();
    }

    fn is_expired(&self, now: std::time::Instant) -> bool {
        let duration = now - self.last_ping;
        duration > Self::EXPIRED
    }
}

#[derive(Debug)]
enum Message {
    Login((SocketAddr, String)),
    Chat((SocketAddr, String)),
    Logout(SocketAddr),
    PingReceived(SocketAddr),
    Tick,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:35600").await?);

    let (sender, mut receiver) = mpsc::unbounded_channel();

    // process message channel to user
    let send_socket = Arc::clone(&socket);
    tokio::spawn(async move {
        let mut users = Users::new();

        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::Login((addr, name)) => {
                    users.add_user(addr, name);
                }
                Message::Logout(addr) => {
                    users.remove_user(addr);
                }
                Message::Chat((addr, contents)) => {
                    let name = users.get_name(addr);
                    if let Some(name) = name {
                        users.send(&send_socket, &name, &contents).await;
                    }
                }
                Message::PingReceived(addr) => {
                    users.ping_received(addr);
                }
                Message::Tick => {
                    users.tick(&send_socket).await;
                }
            }
        }
    });

    // timer (ping)
    let ping_channel = sender.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            ping_channel.send(Message::Tick).unwrap();
        }
    });

    loop {
        let mut buf = [0; MAX_PACKET_SIZE];
        let received = socket.recv_from(&mut buf).await;
        if let Err(err) = received {
            if err.kind() == std::io::ErrorKind::ConnectionReset {
                continue;
            }
            eprintln!("recv_from err: {:?}", err);
            continue;
        }

        let (size, client) = received.unwrap();
        if size == 0 {
            eprintln!("disconnected. {:?}", client);
            let msg = Message::Logout(client);
            sender.send(msg).unwrap();
            continue;
        }

        let read_buf = &buf[..size];
        let read_packet = serde_json::from_slice::<Packets>(read_buf);
        if let Err(err) = read_packet {
            eprintln!("err: {:?}", err);
            continue;
        }

        let read_packet = read_packet.unwrap();
        match read_packet {
            Packets::LoginReq(login_req) => {
                let msg = Message::Login((client, login_req.name));
                let result = sender.send(msg);
                if let Err(err) = result {
                    eprintln!("send to channel error. {:?}", err);
                }
            }
            Packets::ChatReq(chat_req) => {
                let msg = Message::Chat((client, chat_req.contents));
                sender.send(msg).unwrap();
            }
            Packets::Ping => {
                let msg = Message::PingReceived(client);
                sender.send(msg).unwrap();
            }
            _ => {}
        }
    }
}
