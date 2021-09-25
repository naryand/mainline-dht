use crate::{
    dht::{IpPort, MainlineDHT, Node, NodeId, ID_BYTES},
    model::{Arguments, Message, Returns, Type},
};

use std::{collections::HashMap, convert::TryInto, io, net::SocketAddr, sync::Arc};

use bencode::{decode::from_bytes, encode};
use crc32c::crc32c;
use tokio::{
    net::UdpSocket,
    sync::{Mutex, OnceCell},
};

const RECV_BUF: usize = 2000;
pub const PORT: u16 = 25565;

pub struct KRPC {
    id: OnceCell<NodeId>,
    socket: UdpSocket,
    tx: Transactions,
}

type Transactions = Mutex<HashMap<[u8; 2], SocketAddr>>;

impl KRPC {
    pub async fn new() -> Arc<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", PORT)).await.unwrap();
        Arc::new(Self {
            id: OnceCell::new(),
            socket,
            tx: Mutex::new(HashMap::new()),
        })
    }

    pub fn start_listener(self: Arc<Self>, dht: Arc<Mutex<MainlineDHT>>) {
        tokio::task::spawn(async move {
            loop {
                let dht_ = Arc::clone(&dht);
                let krpc_ = Arc::clone(&self);
                match krpc_.listen(dht_).await {
                    Ok(_) => {}
                    Err(e) => eprintln!("{:?}", e),
                }
            }
        });
    }

    fn get_node_id(id: &[u8]) -> Result<[u8; ID_BYTES], String> {
        let mut node_id: [u8; ID_BYTES] = id
            .try_into()
            .map_err(|e| format!("node_id try_into fail {}", e))?;
        node_id.reverse();
        Ok(node_id)
    }

    pub async fn listen(self: Arc<Self>, dht: Arc<Mutex<MainlineDHT>>) -> Result<(), String> {
        let mut buf = vec![0u8; RECV_BUF];
        let (len, addr) = self
            .socket
            .recv_from(&mut buf)
            .await
            .map_err(|e| e.to_string())?;
        buf.truncate(len);

        let msg = from_bytes::<Message>(&buf).map_err(|e| e.to_string())?;
        let _txid: [u8; 2] = msg
            .t
            .try_into()
            .map_err(|e| format!("txid try_into fail {}", e))?;

        match msg.mtype {
            Type::Query { a, .. } => match a {
                Arguments::AnnouncePeer {
                    id,
                    implied_port,
                    info_hash,
                    port,
                    token,
                } => {}
                Arguments::GetPeers { id, info_hash } => {}
                Arguments::FindNode { id, target } => {}
                Arguments::Ping { id } => {
                    let node = Node {
                        id: NodeId(Self::get_node_id(id)?),
                        ip_port: (addr.ip(), addr.port()),
                    };

                    {
                        let mut dht = dht.lock().await;
                        dht.insert_node(node);
                    }
                }
            },
            Type::Response { r } => {
                let mut idx = 0;
                for (w, (i, _)) in buf.windows(4).zip(buf.iter().enumerate()) {
                    if w == b"2:ip" {
                        idx = i + 4;
                        break;
                    }
                }

                if idx > 0 {
                    let ip = &buf[idx..idx + 6];
                    let ip = u32::from_ne_bytes(ip[2..6].try_into().unwrap());

                    let rand = rand::random::<u32>() & 0xff;
                    let r_bits = rand & 0x7;
                    let crc = crc32c(&((ip & 0x030f3fff) | (r_bits << 29)).to_ne_bytes());

                    let mut node_id = rand::random::<[u8; ID_BYTES]>();
                    node_id[0] = ((crc >> 24) & 0xff) as u8;
                    node_id[1] = ((crc >> 16) & 0xff) as u8;
                    node_id[2] = ((crc >> 8) & 0xf8) as u8 | (rand::random::<u8>() & 0x7);
                    node_id[19] = rand as u8;
                    let _ = self.id.set(NodeId(node_id));
                }

                match r {
                    Returns::GetPeers {
                        id,
                        token,
                        values_nodes,
                    } => {}
                    Returns::FindNode { nodes, .. } => {
                        let contacts: Vec<Vec<u8>> = nodes.chunks(26).map(|x| x.to_vec()).collect();

                        for contact in contacts {
                            let node = Node::from_be_bytes(contact);
                            let krpc_ = Arc::clone(&self);

                            let _ = krpc_.ping(node.ip_port).await;
                        }
                    }
                    Returns::Ping { id } => {
                        let node = Node {
                            id: NodeId(Self::get_node_id(id)?),
                            ip_port: (addr.ip(), addr.port()),
                        };

                        {
                            let mut dht = dht.lock().await;
                            dht.insert_node(node);
                        }
                    }
                    Returns::AnnouncePeer { .. } => unreachable!(),
                }
            }
            Type::Error { e } => {
                return Err(format!(
                    "{:?} {}",
                    e.0,
                    std::str::from_utf8(e.1).map_err(|e| format!("from_utf8 {}", e))?
                ))
            }
        }

        Ok(())
    }

    pub async fn ping(self: Arc<Self>, ip_port: IpPort) -> io::Result<()> {
        // check for collisions
        let mut txid;
        let addr = SocketAddr::from(ip_port);
        loop {
            txid = rand::random::<[u8; 2]>();
            let result;
            {
                let tx = self.tx.lock().await;
                result = tx.get(&txid).is_none();
            }
            match result {
                false => continue,
                true => {
                    let mut tx = self.tx.lock().await;
                    tx.insert(txid, addr);
                    break;
                }
            }
        }

        let msg = Message {
            t: &txid,
            mtype: Type::Query {
                a: Arguments::Ping {
                    id: match self.id.initialized() {
                        true => &self.id.get().unwrap().0,
                        false => &[0u8; 20],
                    },
                },
                q: b"ping",
            },
            y: b"q",
        };

        let bytes = encode::to_bytes(&msg).unwrap();
        self.socket.send_to(&bytes, ip_port).await?;

        Ok(())
    }

    pub async fn find_node(self: Arc<Self>, ip_port: IpPort, id: &NodeId) -> io::Result<()> {
        // check for collisions
        let mut txid;
        let addr = SocketAddr::from(ip_port);
        loop {
            txid = rand::random::<[u8; 2]>();
            let result;
            {
                let tx = self.tx.lock().await;
                result = tx.get(&txid).is_none();
            }
            match result {
                false => continue,
                true => {
                    let mut tx = self.tx.lock().await;
                    tx.insert(txid, addr);
                    break;
                }
            }
        }
        let mut target = id.0.to_vec();
        target.reverse();

        let msg = Message {
            t: &txid,
            mtype: Type::Query {
                a: Arguments::FindNode {
                    id: match self.id.initialized() {
                        true => &self.id.get().unwrap().0,
                        false => &[0u8; 20],
                    },
                    target: &target,
                },
                q: b"find_node",
            },
            y: b"q",
        };

        let bytes = encode::to_bytes(&msg).unwrap();
        self.socket.send_to(&bytes, ip_port).await?;

        Ok(())
    }
}
