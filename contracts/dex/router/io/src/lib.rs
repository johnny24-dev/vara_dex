#![no_std]

use dex_factory_io::{Action as FactoryAction, Error as FactoryError, Event as FactoryEvent};
use dex_io::hidden::{calculate_in_amount, calculate_out_amount};
use dex_io::{Action as PairAction, Error as PairError, Event as PairEvent};
use gmeta::{InOut, Metadata, Out};
use gstd::{errors::Error as GstdError, ActorId};
use gstd::{
    errors::Result,
    msg::{self, CodecMessageFuture},
    prelude::*,
};
pub struct ContractMetadata;

impl Metadata for ContractMetadata {
    type Init = InOut<Initialize, Result<(), Error>>;
    type Handle = InOut<Action, Result<Event, Error>>;
    type Reply = ();
    type Others = ();
    type Signal = ();
    type State = Out<State>;
}

/// The contract state.
///
/// For more info about fields, see [`Initialize`].
#[derive(Default, Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub struct State {
    pub factory: ActorId,
    pub wvara: ActorId,
}

/// Initializes the contract.
#[derive(
    Default, Encode, Decode, TypeInfo, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash,
)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub struct Initialize {
    /// [`ActorId`] of the Factory contract.
    pub factory: ActorId,
    /// [`ActorId`] of the Wvara contract.
    pub wvara: ActorId,
}

#[derive(Debug, Decode, Encode, TypeInfo)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum FTAction {
    Mint(u128),
    Burn(u128),
    Transfer {
        from: ActorId,
        to: ActorId,
        amount: u128,
    },
    Approve {
        to: ActorId,
        amount: u128,
    },
    TotalSupply,
    BalanceOf(ActorId),
}

#[derive(Debug, Encode, Decode, TypeInfo, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum FTokenEvent {
    Ok,
    Err,
    Balance(u128),
    PermitId(u128),
}

/// Sends the contract info about what it should do.
#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Action {
    AddLiquidity {
        token_a: ActorId,
        token_b: ActorId,
        amount_a_desired: u128,
        amount_b_desired: u128,
        amount_a_min: u128,
        amount_b_min: u128,
        to: ActorId,
        deadline: u64,
    },

    AddLiquidityVARA {
        token: ActorId,
        amount_token_desired: u128,
        amount_token_min: u128,
        amount_vara_min: u128,
        to: ActorId,
        deadline: u64,
    },

    RemoveLiquidity {
        token_a: ActorId,
        token_b: ActorId,
        liquidity: u128,
        amount_a_min: u128,
        amount_b_min: u128,
        to: ActorId,
        deadline: u64,
    },

    RemoveLiquidityVARA {
        token: ActorId,
        liquidity: u128,
        amount_token_min: u128,
        amount_vara_min: u128,
        to: ActorId,
        deadline: u64,
    },

    SwapExactTokensForTokens {
        amount_in: u128,
        amount_out_min: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    },

    SwapTokensForExactTokens {
        amount_out: u128,
        amount_in_max: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    },

    SwapExactVARAForTokens {
        amount_out_min: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    },

    SwapExactTokensForVARA {
        amount_in: u128,
        amount_out_min: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    },

    SwapTokensForExactVARA {
        amount_out: u128,
        amount_in_max: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    },

    SwapVARAForExactTokens {
        amount_out: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    },
}

/// A result of successfully processed [`Action`].
#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Event {
    AddLiquidity {
        token_a: ActorId,
        token_b: ActorId,
        liquidity: u128,
        amount_a: u128,
        amount_b: u128,
    },
    RemoveLiquidity {
        token_a: ActorId,
        token_b: ActorId,
        liquidity: u128,
        amount_a: u128,
        amount_b: u128,
    },
    SwapExactTokensForTokens {
        amount_in: u128,
        amount_out: u128,
        path: Vec<ActorId>,
        amounts: Vec<u128>,
    },
    SwapTokensForExactTokens {
        amount_in: u128,
        amount_out: u128,
        path: Vec<ActorId>,
        amounts: Vec<u128>,
    },
    SwapExactVARAForTokens {
        amount_in: u128,
        amount_out: u128,
        path: Vec<ActorId>,
        amounts: Vec<u128>,
    },
    SwapExactTokensForVARA {
        amount_in: u128,
        amount_out: u128,
        path: Vec<ActorId>,
        amounts: Vec<u128>,
    },
    SwapTokensForExactVARA {
        amount_in: u128,
        amount_out: u128,
        path: Vec<ActorId>,
        amounts: Vec<u128>,
    },
    SwapVARAForExactTokens {
        amount_in: u128,
        amount_out: u128,
        path: Vec<ActorId>,
        amounts: Vec<u128>,
    },
}

/// Error variants of failed [`Action`].
#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, TypeInfo, Hash)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum Error {
    /// See [`GstdError`].
    GstdError(String),
    /// [`msg::source()`](gstd::msg::source) doesn't equal to `fee_to_setter`.
    AccessRestricted,
    /// [`ActorId::zero()`] was found where it's forbidden.
    ZeroActorId,
    /// SFT [`ActorId`]s in a given pair to create the Pair contract are equal.
    IdenticalTokens,
    /// A pair contract with given SFT [`ActorId`]s already exist.
    PairExist,
    LPPairError(dex_io::Error),
    WvaraError(dex_wvara_io::Error),
    FactoryError(dex_factory_io::Error),
    InsufficientAmount,
    Overflow,
    InsufficientLiquidity,
    TransferFailed,
    GetPairFailed,
    DeadlineExceeded,
    RemovedLiquidityFailed,
    PairNotFound,
    AddLiquidityFailed,
    PairCreationFailed,
    DepositVaraFailed,
    SwapFailed,
    InvalidPath,
    GetReservesFailed,
    InsufficientOutputAmount,
    ExcessiveInputAmount,
    WithdrawVaraFailed,
}

impl From<GstdError> for Error {
    fn from(error: GstdError) -> Self {
        Self::GstdError(error.to_string())
    }
}

impl From<dex_io::Error> for Error {
    fn from(error: dex_io::Error) -> Self {
        Self::LPPairError(error)
    }
}

impl From<dex_wvara_io::Error> for Error {
    fn from(error: dex_wvara_io::Error) -> Self {
        Self::WvaraError(error)
    }
}

impl From<dex_factory_io::Error> for Error {
    fn from(error: dex_factory_io::Error) -> Self {
        Self::FactoryError(error)
    }
}

#[doc(hidden)]
pub mod router_utils {
    use super::*;

    pub fn send<T: Decode>(
        to: ActorId,
        payload: impl Encode,
        value: u128,
    ) -> Result<CodecMessageFuture<T>> {
        Ok(msg::send_for_reply_as(to, payload, value, 0)?)
    }

    pub async fn transfer_tokens(
        token: ActorId,
        sender: ActorId,
        recipient: ActorId,
        amount: u128,
    ) -> Result<(), Error> {
        let payload = FTAction::Transfer {
            from: sender,
            to: recipient,
            amount,
        };

        match send(token, payload,0)?.await? {
            FTokenEvent::Ok => Ok(()),
            FTokenEvent::Err => Err(Error::TransferFailed),
            _ => unreachable!("received an unexpected `FTokenEvent` variant"),
        }
    }
    pub fn sort_tokens(token_a: ActorId, token_b: ActorId) -> (ActorId, ActorId) {
        let token_pair = if token_b > token_a {
            (token_b, token_a)
        } else {
            (token_a, token_b)
        };
        token_pair
    }

    pub async fn get_pair(
        factory: ActorId,
        token_a: ActorId,
        token_b: ActorId,
    ) -> Result<ActorId, Error> {
        let (token_0, _) = sort_tokens(token_a, token_b);
        let get_pair_res: Result<FactoryEvent, FactoryError> =
            send(factory, FactoryAction::GetPair(token_a, token_b), 0)?.await?;
        let Ok(FactoryEvent::Pair(pair)) = get_pair_res else {
            return Err(Error::GetPairFailed);
        };
        Ok(pair)
    }

    pub async fn get_reserves(
        factory: ActorId,
        token_a: ActorId,
        token_b: ActorId,
    ) -> Result<(u128, u128), Error> {
        let (token_0, _) = sort_tokens(token_a, token_b);
        let pair = get_pair(factory, token_a, token_b).await?;
        let get_reveres_res: Result<PairEvent, PairError> =
            send(pair, PairAction::GetReserves { token_a, token_b }, 0)?.await?;
        let mut reserves = (0, 0);
        let Ok(PairEvent::GetReserves {
            reserve_a,
            reserve_b,
            block_timestamp_last,
        }) = get_reveres_res
        else {
            return Err(Error::GetReservesFailed);
        };
        if token_a == token_0 {
            reserves = (reserve_a, reserve_b);
        } else {
            reserves = (reserve_b, reserve_a);
        }
        Ok(reserves)
    }

    pub async fn get_amounts_out(
        factory: ActorId,
        amount_in: u128,
        path: Vec<ActorId>,
    ) -> Result<Vec<u128>, Error> {
        if path.len() < 2 {
            return Err(Error::InvalidPath);
        }
        let mut amounts = Vec::with_capacity(path.len());
        amounts[0] = amount_in;
        for i in 0..path.len() - 1 {
            let reserves = get_reserves(factory, path[i], path[i + 1]).await?;
            let next_amount_out = calculate_out_amount(amounts[i], reserves)?;
            amounts[i + 1] = next_amount_out;
        }
        Ok(amounts)
    }

    pub async fn get_amounts_in(
        factory: ActorId,
        amount_out: u128,
        path: Vec<ActorId>,
    ) -> Result<Vec<u128>, Error> {
        if path.len() < 2 {
            return Err(Error::InvalidPath);
        }
        let mut amounts = Vec::with_capacity(path.len());
        amounts[path.len() - 1] = amount_out;
        for i in (1..path.len()).rev() {
            let reserves = get_reserves(factory, path[i - 1], path[i]).await?;
            let next_amount_in = calculate_in_amount(amounts[i], reserves)?;
            amounts[i - 1] = next_amount_in;
        }
        Ok(amounts)
    }
}
