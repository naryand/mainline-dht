use std::{
    collections::{BTreeMap, HashMap},
    convert::TryInto,
    net::IpAddr,
};

pub const ID_BYTES: usize = 20;
const K: usize = 8;

#[derive(Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub [u8; ID_BYTES]);

pub type InfoHash = NodeId;

pub type IpPort = (IpAddr, u16);

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub ip_port: IpPort,
}

impl Node {
    pub fn from_be_bytes(bytes: Vec<u8>) -> Self {
        let mut node_bytes: [u8; ID_BYTES] = bytes[0..ID_BYTES].try_into().unwrap();
        node_bytes.reverse();

        let ip_bytes: [u8; 4] = bytes[20..24].try_into().unwrap();

        let ip = IpAddr::from(ip_bytes);
        let port = u16::from_be_bytes(bytes[24..].try_into().unwrap());

        Node {
            id: NodeId(node_bytes),
            ip_port: (ip, port),
        }
    }
}

#[allow(dead_code)]
pub struct MainlineDHT {
    pub table: BTreeMap<NodeId, Node>,
    hashmap: HashMap<InfoHash, Vec<IpPort>>,
}

#[allow(dead_code)]
impl MainlineDHT {
    pub fn new() -> Self {
        Self {
            table: BTreeMap::new(),
            hashmap: HashMap::new(),
        }
    }

    pub fn insert_node(&mut self, node: Node) {
        let id = node.id.clone();
        self.table.insert(id, node);
    }

    pub fn find_closest(&self, id: &NodeId) -> Vec<Node> {
        let all_ids: Vec<NodeId> = self.table.keys().map(|x| x.clone()).collect();

        let idx = match all_ids.binary_search(id) {
            Ok(i) => i,
            Err(i) => i,
        };
        let start = (idx as isize - (K / 2) as isize).clamp(0, all_ids.len() as isize) as usize;
        let end = (idx as isize + (K / 2) as isize).clamp(0, all_ids.len() as isize) as usize;

        let k_closest_ids = all_ids[start..end].to_vec();
        let mut k_closest = vec![];
        for i in k_closest_ids {
            k_closest.push(self.table.get(&i).unwrap().clone());
        }

        k_closest
    }

    pub fn find_node(&self, id: &NodeId) -> Result<Node, Vec<Node>> {
        match self.table.get(id) {
            Some(node) => Ok(node.clone()),
            None => Err(self.find_closest(id)),
        }
    }

    pub fn find_value(&self, id: &InfoHash) -> Result<Vec<IpPort>, Vec<Node>> {
        match self.hashmap.get(id) {
            Some(val) => Ok(val.clone()),
            None => Err(self.find_closest(id)),
        }
    }

    pub fn store(&mut self, id: &InfoHash, ip_port: IpPort) {
        let _ = match self.hashmap.get(id) {
            Some(val) => {
                let mut new_val = val.clone();
                new_val.push(ip_port);
                self.hashmap.insert(id.clone(), new_val)
            }
            None => self.hashmap.insert(id.clone(), vec![ip_port]),
        };
    }
}
