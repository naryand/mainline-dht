/// Model for KRPC Messages
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Serialize, Deserialize, Debug)]
pub struct Message<'a> {
    #[serde(borrow)]
    #[serde(with = "serde_bytes")]
    pub t: &'a [u8],

    #[serde(borrow)]
    #[serde(flatten)]
    pub mtype: Type<'a>,

    #[serde(borrow)]
    #[serde(with = "serde_bytes")]
    pub y: &'a [u8],
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Type<'a> {
    Query {
        #[serde(borrow)]
        a: Arguments<'a>,

        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        q: &'a [u8],
    },
    Response {
        // #[serde(borrow)]
        // #[serde(with = "serde_bytes")]
        // ip: &'a [u8],
        #[serde(borrow)]
        r: Returns<'a>,
    },
    Error {
        #[serde(borrow)]
        e: Errors<'a>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Arguments<'a> {
    AnnouncePeer {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        id: &'a [u8],

        implied_port: u64,

        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        info_hash: &'a [u8],

        port: u64,

        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        token: &'a [u8],
    },
    FindNode {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        id: &'a [u8],

        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        target: &'a [u8],
    },
    GetPeers {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        id: &'a [u8],

        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        info_hash: &'a [u8],
    },
    Ping {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        id: &'a [u8],
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Returns<'a> {
    GetPeers {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        id: &'a [u8],

        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        token: &'a [u8],

        #[serde(borrow)]
        #[serde(flatten)]
        values_nodes: ValuesNodes<'a>,
    },
    FindNode {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        id: &'a [u8],

        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        nodes: &'a [u8],
    },
    Ping {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        id: &'a [u8],
    },
    AnnouncePeer {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        id: &'a [u8],
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ValuesNodes<'a> {
    Values {
        #[serde(borrow)]
        values: Vec<Slice<'a>>,
    },
    Nodes {
        #[serde(borrow)]
        #[serde(with = "serde_bytes")]
        nodes: &'a [u8],
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Slice<'a>(
    #[serde(borrow)]
    #[serde(with = "serde_bytes")]
    pub &'a [u8],
);

#[derive(Serialize, Deserialize, Debug)]
pub struct Errors<'a>(pub ErrorCode, pub &'a [u8]);

#[derive(Serialize_repr, Deserialize_repr, Debug)]
#[repr(u64)]
pub enum ErrorCode {
    GenericError = 201,
    ServerError = 202,
    ProtocolError = 203,
    MethodUnknown = 204,
}
