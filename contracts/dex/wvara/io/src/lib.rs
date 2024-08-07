#![no_std]

use gmeta::{InOut, Metadata, Out};
use gstd::{errors::Error as GstdError, prelude::*, ActorId};

pub struct ContractMetadata;

impl Metadata for ContractMetadata {
    type Init = ();
    type Handle = InOut<Action, Result<Event, Error>>;
    type Reply = ();
    type Others = ();
    type Signal = ();
    type State = Out<WVaraState>;
}

#[derive(Default, Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub struct WVaraState {
    pub name: String,
    pub symbol: String,
    pub decimals: u64,
    pub balance_of: Vec<(ActorId, u128)>,
    pub allowance: Vec<((ActorId, ActorId), u128)>,
    pub total_supply: u128,
}

impl WVaraState {
    pub fn balance_of(&self, actor: ActorId) -> u128 {
        self.balance_of
            .iter()
            .find_map(|(actor_id, balance)| (*actor_id == actor).then_some(*balance))
            .unwrap_or_default()
    }

    pub fn allowance(&self, owner: ActorId, spender: ActorId) -> u128 {
        self.allowance
            .iter()
            .find_map(|((owner_id, spender_id), amount)| {
                (*owner_id == owner && *spender_id == spender).then_some(*amount)
            })
            .unwrap_or_default()
    }
}

/// Sends the contract info about what it should do.
#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Action {
    Deposit,
    Withdraw {
        to: ActorId,
        amount: u128,
    },
    Approve {
        spender: ActorId,
        amount: u128,
    },
    Transfer {
        from:ActorId,
        to: ActorId,
        amount: u128,
    },
    TransferFrom {
        from: ActorId,
        to: ActorId,
        amount: u128,
    },
    BalanceOf(ActorId),
}

/// A result of successfully processed [`Action`].
#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Event {
    Approve {
        owner: ActorId,
        spender: ActorId,
        amount: u128,
    },
    Transfer {
        from: ActorId,
        to: ActorId,
        amount: u128,
    },
    Deposit {
        from: ActorId,
        amount: u128,
    },
    Withdraw {
        to: ActorId,
        amount: u128,
    },
    Balance(u128),
}

/// Fungible token error variants.
#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Error {
    /// Token owner doesn't have a sufficient amount of tokens. Or there was the
    /// [`Amount`] overflow during token minting.
    InsufficientAmount,
    GstdError(String),
    /// [`msg::source()`] or operator doesn't have a sufficient allowance of
    /// tokens. Or there was the [`Amount`] overflow during allowance
    /// increasing.
    InsufficientAllowance,
    /// A recipient/operator address is [`ActorId::zero()`].
    ZeroRecipientAddress,
    /// A sender address is [`ActorId::zero()`].
    ZeroSenderAddress,
    InsufficientBalance,
    SendFailed,
    InsufficientTotalSupply,
}

impl From<GstdError> for Error {
    fn from(error: GstdError) -> Self {
        Self::GstdError(error.to_string())
    }
}
