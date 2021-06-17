use rand::prelude::*;
use std::io::BufRead;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

use udp_chat::{ChatReqPacket, LoginReqPacket, Packets, MAX_PACKET_SIZE};

fn main() -> std::io::Result<()> {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
    socket.connect("127.0.0.1:35600")?;

    let login = LoginReqPacket {
        name: format!("client#{}", random::<u32>()),
    };
    let login_packet = Packets::LoginReq(login);

    let json = serde_json::to_string(&login_packet).unwrap();
    let bytes = json.as_bytes();

    socket.send(bytes)?;

    let last_ping = Arc::new(Mutex::new(std::time::Instant::now()));

    // receive thread
    let receive_socket = Arc::clone(&socket);
    let last_ping_cloned = Arc::clone(&last_ping);
    std::thread::spawn(move || loop {
        let mut buf = [0; MAX_PACKET_SIZE];
        let result = receive_socket.recv(&mut buf);
        match result {
            Ok(size) => {
                let read_buf = &buf[..size];
                let read_packet = serde_json::from_slice::<Packets>(read_buf);
                match read_packet {
                    Ok(chat_packet) => match chat_packet {
                        Packets::ChatNotify(chat_notify) => {
                            println!("{}: {}", chat_notify.name, chat_notify.contents);
                        }
                        Packets::Ping => {
                            let mut locked = last_ping_cloned.lock().unwrap();
                            *locked = std::time::Instant::now();
                        }
                        _ => {}
                    },
                    Err(err) => {
                        eprintln!("read packet error. {:?}", err);
                    }
                }
            }
            Err(err) => {
                eprintln!("recv error. {:?}", err);
            }
        }
    });

    // timer thread
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    });

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        if let Ok(line) = line {
            let chat_req = ChatReqPacket { contents: line };
            let packet = Packets::ChatReq(chat_req);
            let json = serde_json::to_string(&packet).unwrap();
            let bytes = json.as_bytes();
            if bytes.len() >= MAX_PACKET_SIZE {
                eprintln!("packet size overflow. {}", bytes.len());
                continue;
            }

            let result = socket.send(bytes);
            if let Err(err) = result {
                eprintln!("fail to send chat. {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
