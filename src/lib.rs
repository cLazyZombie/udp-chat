use serde::{Deserialize, Serialize};

pub const MAX_PACKET_SIZE: usize = 2048;

#[derive(Debug, Serialize, Deserialize)]
pub enum Packets {
    LoginReq(LoginReqPacket),
    ChatReq(ChatReqPacket),
    ChatNotify(ChatNotifyPacket),
    Ping,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginReqPacket {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatReqPacket {
    pub contents: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatNotifyPacket {
    pub name: String,
    pub contents: String,
}
