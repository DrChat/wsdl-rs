#![doc = include_str!("../README.md")]
#![feature(try_find)]
mod wsdl;

pub use self::wsdl::{
    WsBinding, WsDefinitions, WsError, WsMessage, WsMessagePart, WsPortOperation, WsPortType,
    WsService, WsServicePort, WsTypes,
};

/// Re-export the roxmltree crate.
pub use roxmltree;
