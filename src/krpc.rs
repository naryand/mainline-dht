use crate::dht::{IpPort, MainlineDHT, Node, NodeId, ID_BYTES};

use std::{
    collections::{BTreeMap, HashMap},
    convert::TryInto,
    io,
    net::SocketAddr,
    sync::Arc,
};

use bencode::{decode::parse, encode::encode, Item};
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

#[allow(unused)]
#[derive(Debug)]
enum Msg {
    Ping,
    FindNode,
    GetPeers,
    AnnouncePeer,
}

type Transactions = Mutex<HashMap<[u8; 2], (SocketAddr, Msg)>>;

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

    pub async fn listen(self: Arc<Self>, dht: Arc<Mutex<MainlineDHT>>) -> Result<(), String> {
        let mut buf = vec![0u8; RECV_BUF];
        let (len, addr) = self
            .socket
            .recv_from(&mut buf)
            .await
            .map_err(|e| e.to_string())?;
        buf.truncate(len);

        let response = parse(&mut buf).ok_or("parse error")?[0].get_dict();
        match response
            .get("y".as_bytes())
            .ok_or("no y key")?
            .get_str()
            .as_slice()
        {
            // handle inbound queries
            b"q" => match response.get("q".as_bytes()).unwrap().get_str().as_slice() {
                // handle incomng ping
                b"ping" => {
                    let args = response.get("a".as_bytes()).unwrap().get_dict();
                    let mut id = args.get("id".as_bytes()).unwrap().get_str();
                    id.reverse();

                    let node = Node {
                        id: NodeId(id.try_into().unwrap()),
                        ip_port: (addr.ip(), addr.port()),
                    };

                    {
                        let mut dht = dht.lock().await;
                        dht.insert_node(node);
                    }
                }
                _ => {}
            },
            // handle response from sent query
            b"r" => {
                let r = response.get("r".as_bytes()).unwrap().get_dict();
                let txid: [u8; 2] = response
                    .get("t".as_bytes())
                    .unwrap()
                    .get_str()
                    .as_slice()
                    .try_into()
                    .unwrap();
                let entry;
                {
                    let mut tx = self.tx.lock().await;
                    entry = tx.remove(&txid).ok_or("tx remove fail")?;
                }
                let (addr, msg);
                addr = entry.0;
                msg = entry.1;

                match msg {
                    // handle ping response
                    Msg::Ping => {
                        match response.get("ip".as_bytes()) {
                            Some(item) => {
                                let ip =
                                    u32::from_ne_bytes(item.get_str()[0..4].try_into().unwrap());

                                let rand = rand::random::<u32>() & 0xff;
                                let r = rand & 0x7;
                                let crc = crc32c(&((ip & 0x030f3fff) | (r << 29)).to_ne_bytes());

                                let mut node_id = rand::random::<[u8; ID_BYTES]>();
                                node_id[0] = ((crc >> 24) & 0xff) as u8;
                                node_id[1] = ((crc >> 16) & 0xff) as u8;
                                node_id[2] =
                                    ((crc >> 8) & 0xf8) as u8 | (rand::random::<u8>() & 0x7);
                                node_id[19] = rand as u8;
                                let _ = self.id.set(NodeId(node_id));
                            }
                            None => {}
                        }

                        let id = r.get("id".as_bytes()).unwrap().get_str();
                        let node = Node {
                            id: NodeId(
                                id.try_into()
                                    .map_err(|e| format!("NodeId try_into fail {:?}", e))?,
                            ),
                            ip_port: (addr.ip(), addr.port()),
                        };

                        {
                            let mut dht = dht.lock().await;
                            dht.insert_node(node);
                        }
                    }
                    Msg::FindNode => match r.get("target".as_bytes()) {
                        Some(item) => {
                            let contact = item.get_str();
                            let node = Node::from_be_bytes(contact);

                            let krpc_ = Arc::clone(&self);

                            let _ = krpc_.ping(node.ip_port).await;
                        }
                        None => match r.get("nodes".as_bytes()) {
                            Some(item) => {
                                let contacts: Vec<Vec<u8>> =
                                    item.get_str().chunks(26).map(|x| x.to_vec()).collect();

                                for contact in contacts {
                                    let node = Node::from_be_bytes(contact);
                                    let krpc_ = Arc::clone(&self);

                                    let _ = krpc_.ping(node.ip_port).await;
                                }
                            }
                            None => return Err("no target or nodes key".to_string()),
                        },
                    },
                    Msg::GetPeers => todo!(),
                    Msg::AnnouncePeer => todo!(),
                }
            }
            // handle errors
            b"e" => {}

            _ => unreachable!(),
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
                    tx.insert(txid, (addr, Msg::Ping));
                    break;
                }
            }
        }

        let mut map: BTreeMap<Vec<u8>, Item> = BTreeMap::new();
        map.insert(b"t".to_vec(), Item::String(txid.to_vec()));
        map.insert(b"y".to_vec(), Item::String(b"q".to_vec()));
        map.insert(b"q".to_vec(), Item::String(b"ping".to_vec()));

        let mut args: BTreeMap<Vec<u8>, Item> = BTreeMap::new();
        let id = [0u8; 20];
        args.insert(b"id".to_vec(), Item::String(id.to_vec()));
        map.insert(b"a".to_vec(), Item::Dict(args));

        let bytes = encode(vec![Item::Dict(map)]);
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
                    tx.insert(txid, (addr, Msg::FindNode));
                    break;
                }
            }
        }

        let mut map: BTreeMap<Vec<u8>, Item> = BTreeMap::new();
        map.insert(b"t".to_vec(), Item::String(txid.to_vec()));
        map.insert(b"y".to_vec(), Item::String(b"q".to_vec()));
        map.insert(b"q".to_vec(), Item::String(b"find_node".to_vec()));

        let mut args: BTreeMap<Vec<u8>, Item> = BTreeMap::new();
        args.insert(
            b"id".to_vec(),
            Item::String(self.id.get().unwrap().0.to_vec()),
        );
        let mut target = id.0.to_vec();
        target.reverse();
        args.insert(b"target".to_vec(), Item::String(target));
        map.insert(b"a".to_vec(), Item::Dict(args));

        let bytes = encode(vec![Item::Dict(map)]);

        self.socket.send_to(&bytes, ip_port).await?;

        Ok(())
    }
}
