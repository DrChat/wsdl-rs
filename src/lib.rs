#![doc = include_str!("../README.md")]
#![feature(try_find)]
mod wsdl;

pub use self::wsdl::{
    WsBinding, WsError, WsMessage, WsMessagePart, WsPortOperation, WsPortType, WsService,
    WsServicePort, Wsdl,
};

/// Re-export the roxmltree crate.
pub use roxmltree;
