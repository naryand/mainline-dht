use std::{
    collections::{BTreeMap, HashMap},
    net::IpAddr,
};

const ID_BYTES: usize = 20;
#[derive(Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NodeId([u8; ID_BYTES]);

type InfoHash = NodeId;

#[derive(Clone)]
struct IpPort {
    ip: IpAddr,
    port: u16,
}

#[derive(Clone)]
struct Node {
    id: NodeId,
    ip_port: IpPort,
}

struct MainlineDHT {
    table: BTreeMap<NodeId, Node>,
    hashmap: HashMap<InfoHash, Vec<IpPort>>,
    k: usize,
}

impl MainlineDHT {
    fn insert_node(&mut self, node: Node) {
        let id = node.id.clone();
        self.table.insert(id, node);
    }

    fn find_closest(&self, id: &NodeId) -> Vec<Node> {
        let all_ids: Vec<NodeId> = self.table.keys().map(|x| x.clone()).collect();

        let idx = all_ids.binary_search(id).unwrap();
        let start = (idx - (self.k / 2)).clamp(0, all_ids.len());
        let end = (idx + (self.k / 2)).clamp(0, all_ids.len());

        let k_closest_ids = all_ids[start..end].to_vec();
        let mut k_closest = vec![];
        for i in k_closest_ids {
            k_closest.push(self.table.get(&i).unwrap().clone());
        }

        k_closest
    }

    fn find_node(&self, id: &NodeId) -> Result<Node, Vec<Node>> {
        match self.table.get(id) {
            Some(node) => Ok(node.clone()),
            None => Err(self.find_closest(id)),
        }
    }

    fn find_value(&self, id: &InfoHash) -> Result<Vec<IpPort>, Vec<Node>> {
        match self.hashmap.get(id) {
            Some(val) => Ok(val.clone()),
            None => Err(self.find_closest(id)),
        }
    }

    fn store(&mut self, id: &InfoHash, ip_port: IpPort) {
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

fn main() {
    println!("Hello, world!");
}
