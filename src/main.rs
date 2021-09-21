mod dht;
mod krpc;

use krpc::KRPC;
use dht::{IpPort, MainlineDHT, NodeId, ID_BYTES};

use std::{io, net::IpAddr, sync::Arc};

use tokio::sync::Mutex;


#[tokio::main]
async fn main() -> io::Result<()> {
    let krpc = KRPC::new().await;
    let dht = Arc::new(Mutex::new(MainlineDHT::new()));

    let ip_port = (IpAddr::from([67, 215, 246, 10]), 6881);

    let krpc_ = Arc::clone(&krpc);
    krpc_.ping(ip_port).await?;

    let dht_ = Arc::clone(&dht);
    let krpc_ = Arc::clone(&krpc);
    krpc_.start_listener(dht_);

    loop {
        let node_id = NodeId(rand::random::<[u8; ID_BYTES]>());
        let addrs;
        {
            let dht = dht.lock().await;
            addrs = dht
                .find_closest(&node_id)
                .iter()
                .map(|x| x.ip_port)
                .collect::<Vec<IpPort>>();
        }
        for addr in addrs {
            let id = node_id.clone();
            let krpc_ = Arc::clone(&krpc);
            let _ = krpc_.find_node(addr, &id).await;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        {
            let dht = dht.lock().await;
            // println!("{:#?}", dht.find_closest(&node_id));
            println!("{:?}", dht.table.len());
        }
    }
}
