#![no_std]

use gmeta::{InOut, Metadata, Out};
use gstd::{errors::Error as GstdError, prelude::*, ActorId, CodeId};

pub struct ContractMetadata;

impl Metadata for ContractMetadata {
    type Init = ();
    type Handle = InOut<Action, Result<Event, Error>>;
    type Reply = ();
    type Others = ();
    type Signal = ();
    type State = ();
}


#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Action {
   Transfer { token:ActorId, to: ActorId, amount: u128 },
   TransferWVara { wvara: ActorId, to: ActorId, amount: u128 },
}

/// A result of successfully processed [`Action`].
#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Event {
    Transfer { from: ActorId, to: ActorId, amount: u128 }
}

/// Error variants of failed [`Action`].
#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Error {
    TransferFailed,
    GstdError(String),
    TransferFailedAction,
    TransferWVaraFailedAction,
}

impl From<GstdError> for Error {
    fn from(error: GstdError) -> Self {
        Self::GstdError(error.to_string())
    }
}