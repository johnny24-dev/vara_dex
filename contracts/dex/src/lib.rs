#![no_std]

use dex_factory_io::{Action as FactoryAction, Error as FactoryError, Event as FactoryEvent};
use dex_io::{
    hidden::{
        calculate_in_amount, calculate_out_amount, quote, quote_reserve_unchecked, U256PairTuple,
    },
    *,
};
use gear_lib::{
    tokens::fungible::FTState,
    tx_manager::{self, ActionKind, Stepper, TransactionManager},
};
use gstd::{errors::Result, exec, msg, prelude::*, ActorId};
use primitive_types::U256;

mod utils;

fn state_mut() -> &'static mut (Contract) {
    unsafe { STATE.as_mut().expect("state isn't initialized") }
}

static mut STATE: Option<Contract> = None;

#[derive(Default)]
struct Contract {
    factory: ActorId,
    router: ActorId,
    token: (ActorId, ActorId),
    reserve: (u128, u128),
    cumulative_price: (U256, U256),
    last_block_ts: u64,
    k_last: U256,
    ft_state: FTState,
}

impl Contract {
    // only router can call this function
    async fn add_liquidity(
        &mut self,
        desired_amount: (u128, u128),
        min_amount: (u128, u128),
        to: ActorId,
    ) -> Result<Event, Error> {
        if self.router != msg::source() {
            return Err(Error::InvalidRouter);
        }
        // Calculating an input amount
        let amount = if self.reserve == (0, 0) {
            desired_amount
        } else {
            let optimal_amount_b = quote(desired_amount.0, self.reserve)?;

            if optimal_amount_b <= desired_amount.1 {
                if optimal_amount_b < min_amount.1 {
                    return Err(Error::InsufficientLatterAmount);
                }

                (desired_amount.0, optimal_amount_b)
            } else {
                let optimal_amount_a =
                    quote_reserve_unchecked(desired_amount.1, (self.reserve.1, self.reserve.0))?;

                if optimal_amount_a < min_amount.0 {
                    return Err(Error::InsufficientFormerAmount);
                }

                (optimal_amount_a, desired_amount.1)
            }
        };

        let balance = if let (Some(balance_a), Some(balance_b)) = (
            self.reserve.0.checked_add(amount.0),
            self.reserve.1.checked_add(amount.1),
        ) {
            (balance_a, balance_b)
        } else {
            return Err(Error::Overflow);
        };

        let (is_fee_on, fee_receiver, fee) = self.calculate_fee().await?;
        let U256PairTuple(amount_u256) = amount.into();
        let program_id = exec::program_id();

        // Calculating liquidity
        let (liquidity, event) = if self.ft_state.total_supply().is_zero() {
            // First minting

            let liquidity = (amount_u256.0 * amount_u256.1)
                .integer_sqrt()
                .checked_sub(MINIMUM_LIQUIDITY.into())
                .ok_or(Error::InsufficientAmount)?;

            let event = self
                .update_liquidity(program_id, to, amount, balance, liquidity)
                .await?;

            // Locking the `MINIMUM_LIQUIDITY` for safer calculations during
            // further operations.
            self.ft_state
                .mint(program_id, MINIMUM_LIQUIDITY.into())
                .expect("unchecked condition occurred for `FTState`");

            (liquidity, event)
        } else {
            // Subsequent mintings

            // Checking for an overflow on adding `fee` to `total_supply.`
            let total_supply = self
                .ft_state
                .total_supply()
                .checked_add(fee)
                .ok_or(Error::Overflow)?;
            let (Some(numerator_a), Some(numerator_b)) = (
                amount_u256.0.checked_mul(total_supply),
                amount_u256.1.checked_mul(total_supply),
            ) else {
                return Err(Error::Overflow);
            };
            let U256PairTuple(reserve) = self.reserve.into();
            let liquidity = cmp::min(numerator_a / reserve.0, numerator_b / reserve.1);

            // Checking for an overflow on adding `liquidity` to `total_supply.`
            if total_supply.checked_add(liquidity).is_none() {
                return Err(Error::Overflow);
            }

            let event = self
                .update_liquidity(program_id, to, amount, balance, liquidity)
                .await?;

            if !fee.is_zero() {
                self.ft_state
                    .mint(fee_receiver, fee)
                    .expect("unchecked overflow occurred for `FTState`");
            }

            (liquidity, event)
        };

        if is_fee_on {
            let U256PairTuple(balance) = balance.into();
            self.k_last = balance.0 * balance.1;
        }
        self.ft_state
            .mint(to, liquidity)
            .expect("unchecked condition occurred for `FTState`");

        Ok(event)
    }

    async fn update_liquidity(
        &mut self,
        program_id: ActorId,
        msg_source: ActorId,
        amount: (u128, u128),
        balance: (u128, u128),
        liquidity: U256,
    ) -> Result<Event, Error> {
        if liquidity.is_zero() {
            return Err(Error::InsufficientLiquidity);
        }

        self.update(balance);

        Ok(Event::AddedLiquidity {
            sender: msg_source,
            amount_a: amount.0,
            amount_b: amount.1,
            liquidity,
        })
    }

    async fn calculate_fee(&self) -> Result<(bool, ActorId, U256), Error> {
        let fee_to_result: Result<FactoryEvent, FactoryError> =
            utils::send(self.factory, FactoryAction::GetFeeTo)?.await?;
        let Ok(FactoryEvent::FeeToSet(fee_receiver)) = fee_to_result else {
            return Err(Error::FeeToGettingFailed);
        };

        let is_fee_on = !fee_receiver.is_zero();
        let mut fee = U256::zero();

        if is_fee_on && !self.k_last.is_zero() {
            let U256PairTuple(reserve) = self.reserve.into();
            let root_k = (reserve.0 * reserve.1).integer_sqrt();
            let root_k_last = self.k_last.integer_sqrt();

            if root_k > root_k_last {
                let numerator = self
                    .ft_state
                    .total_supply()
                    .checked_mul(root_k - root_k_last)
                    .ok_or(Error::Overflow)?;
                // Shouldn't overflow.
                let denominator = root_k * 5 + root_k_last;

                fee = numerator / denominator;
            }
        }

        Ok((is_fee_on, fee_receiver, fee))
    }

    // user transfer liquidity to router and router call this function
    async fn remove_liquidity(
        &mut self,
        liquidity: U256,
        min_amount: (u128, u128),
        to: ActorId,
    ) -> Result<Event, Error> {
        // check caller liquidity
        if self.ft_state.balance_of(msg::source()) < liquidity {
            return Err(Error::InsufficientLiquidity);
        }

        let (is_fee_on, fee_receiver, fee) = self.calculate_fee().await?;
        let U256PairTuple(reserve) = self.reserve.into();

        // Calculating an output amount
        let amount = if let (Some(amount_a), Some(amount_b)) = (
            liquidity.checked_mul(reserve.0),
            liquidity.checked_mul(reserve.1),
        ) {
            // Checking for an overflow on adding `fee` to `total_supply.`
            if let Some(total_supply) = self.ft_state.total_supply().checked_add(fee) {
                // Shouldn't be more than u128::MAX, so casting doesn't lose
                // data.
                (
                    (amount_a / total_supply).low_u128(),
                    (amount_b / total_supply).low_u128(),
                )
            } else {
                return Err(Error::Overflow);
            }
        } else {
            return Err(Error::Overflow);
        };

        if amount.0 == 0 || amount.1 == 0 {
            return Err(Error::InsufficientLiquidity);
        }

        if amount.0 < min_amount.0 {
            return Err(Error::InsufficientFormerAmount);
        }

        if amount.1 < min_amount.1 {
            return Err(Error::InsufficientLatterAmount);
        }
        let program_id = exec::program_id();
        

        utils::transfer_tokens(self.token.0, program_id, to, amount.0).await?;
        utils::transfer_tokens(self.token.1, program_id, to, amount.1).await?;

        self.ft_state
            .burn(msg::source(), liquidity)
            .expect("unchecked overflow occurred for `FTState`");
        
        let balance = (self.reserve.0 - amount.0, self.reserve.1 - amount.1);

        if is_fee_on {
            if !fee.is_zero() {
                self.ft_state
                    .mint(fee_receiver, fee)
                    .expect("unchecked overflow occurred for `FTState`");
            }

            let U256PairTuple(balance) = balance.into();

            self.k_last = balance.0 * balance.1;
        }

        self.update(balance);

        Ok(Event::RemovedLiquidity {
            sender: msg::source(),
            amount_a: amount.0,
            amount_b: amount.1,
            to,
        })
    }

    async fn skim(&self, to: ActorId) -> Result<Event, Error> {
        let program_id = exec::program_id();
        let contract_balance = self.balances(program_id).await?;

        let (Some(excess_a), Some(excess_b)) = (
            contract_balance.0.checked_sub(self.reserve.0),
            contract_balance.1.checked_sub(self.reserve.1),
        ) else {
            return Err(Error::Overflow);
        };

        utils::transfer_tokens(self.token.0, program_id, to, excess_a).await?;
        utils::transfer_tokens(self.token.1, program_id, to, excess_b).await?;

        Ok(Event::Skim {
            amount_a: excess_a,
            amount_b: excess_b,
            to,
        })
    }

    async fn sync(&mut self) -> Result<Event, Error> {
        let program_id = exec::program_id();
        let balance = self.balances(program_id).await?;

        self.update(balance);

        Ok(Event::Sync {
            reserve_a: balance.0,
            reserve_b: balance.1,
        })
    }

    async fn balances(&self, program_id: ActorId) -> Result<(u128, u128)> {
        Ok((
            utils::balance_of(self.token.0, program_id).await?,
            utils::balance_of(self.token.1, program_id).await?,
        ))
    }

    fn update(&mut self, balance: (u128, u128)) {
        let block_ts = exec::block_timestamp();
        let time_elapsed = block_ts - self.last_block_ts;

        if time_elapsed > 0 && self.reserve != (0, 0) {
            let U256PairTuple(reserve) = self.reserve.into();
            let calculate_cp = |reserve: (U256, U256)| {
                // The `u64` suffix is needed for a faster conversion.
                ((reserve.1 << U256::from(128u64)) / reserve.0)
                    // TODO: replace `overflowing_mul` with `wrapping_mul`.
                    // At the moment "primitive-types" doesn't have this method.
                    .overflowing_mul(time_elapsed.into())
                    .0
            };

            self.cumulative_price.0 += calculate_cp(reserve);
            self.cumulative_price.1 += calculate_cp((reserve.1, reserve.0));
        }

        self.reserve = balance;
        self.last_block_ts = block_ts;
    }

    fn swap_pattern(&self, kind: SwapKind) -> SwapPattern {
        match kind {
            SwapKind::AForB => SwapPattern {
                token: self.token,
                reserve: self.reserve,
                normalize_balance: convert::identity,
            },
            SwapKind::BForA => SwapPattern {
                token: (self.token.1, self.token.0),
                reserve: (self.reserve.1, self.reserve.0),
                normalize_balance: |amount| (amount.1, amount.0),
            },
        }
    }

    fn check_recipient(&self, recipient: ActorId) -> Result<(), Error> {
        if recipient == self.token.0 || recipient == self.token.1 {
            Err(Error::InvalidRecipient)
        } else {
            Ok(())
        }
    }

    async fn swap(
        &mut self,
        kind: SwapKind,
        (in_amount, out_amount): (u128, u128),
        to: ActorId,
    ) -> Result<Event, Error> {
        let swap_pattern = self.swap_pattern(kind);
        if self.router != msg::source() {
            return Err(Error::InvalidRouter);
        }

        // let SwapPattern {
        //     token: (in_token, out_token),
        //     reserve,
        //     normalize_balance,
        // }: swap_pattern;

        let program_id = exec::program_id();
        // utils::transfer_tokens( in_token, msg_source, program_id, in_amount).await?;

        if let Err(error) =
            utils::transfer_tokens(swap_pattern.token.1, program_id, to, out_amount).await
        {
            utils::transfer_tokens(swap_pattern.token.0, program_id, to, in_amount).await?;

            return Err(error);
        }

        self.update((
            swap_pattern.reserve.0 + in_amount,
            swap_pattern.reserve.1 - out_amount,
        ));

        Ok(Event::Swap {
            sender: msg::source(),
            in_amount,
            out_amount,
            to,
            kind,
        })
    }
}

struct SwapPattern {
    token: (ActorId, ActorId),
    reserve: (u128, u128),
    normalize_balance: fn((u128, u128)) -> (u128, u128),
}

fn check_deadline(deadline: u64) -> Result<(), Error> {
    if exec::block_timestamp() > deadline {
        Err(Error::DeadlineExceeded)
    } else {
        Ok(())
    }
}

#[no_mangle]
extern fn init() {
    let result = process_init();
    let is_err = result.is_err();

    msg::reply(result, 0).expect("failed to encode or reply from `init()`");

    if is_err {
        exec::exit(ActorId::zero());
    }
}

fn process_init() -> Result<(), Error> {
    let Initialize {
        pair: token,
        factory,
        router,
    } = msg::load()?;

    if token.0.is_zero() || token.1.is_zero() {
        return Err(Error::ZeroActorId);
    }

    if token.0 == token.1 {
        return Err(Error::IdenticalTokens);
    }

    unsafe {
        STATE = Some(Contract {
            token,
            factory,
            router,
            ..Default::default()
        });
    };

    Ok(())
}

#[gstd::async_main]
async fn main() {
    msg::reply(process_handle().await, 0).expect("failed to encode or reply `handle()`");
}

async fn process_handle() -> Result<Event, Error> {
    let action = msg::load()?;
    let contract = state_mut();
    let msg_source = msg::source();

    match action {
        InnerAction::AddLiquidity {
            amount_a_desired,
            amount_b_desired,
            amount_a_min,
            amount_b_min,
            to,
            deadline,
        } => {
            check_deadline(deadline)?;

            contract
                .add_liquidity(
                    (amount_a_desired, amount_b_desired),
                    (amount_a_min, amount_b_min),
                    to,
                )
                .await
        },
        InnerAction::RemoveLiquidity {
            liquidity,
            amount_a_min,
            amount_b_min,
            to,
            deadline,
        } => {
            check_deadline(deadline)?;
            contract
                .remove_liquidity(liquidity, (amount_a_min, amount_b_min), to)
                .await
        },
        InnerAction::Swap {
            in_amount,
            out_amount,
            to,
            a_to_b,
        } => {
            let swap_kind = if a_to_b {
                SwapKind::AForB
            } else {
                SwapKind::BForA
            };

            contract.swap(swap_kind, (in_amount, out_amount), to).await
        },
        InnerAction::Skim(to) => contract.skim(to).await,
        InnerAction::Sync => contract.sync().await,
        InnerAction::Transfer { to, amount } => contract
            .ft_state
            .transfer(to, U256::from(amount))
            .map(Into::into)
            .map_err(Into::into),
        InnerAction::GetReserves { token_a, token_b } => {
            if token_a == contract.token.0 && token_b == contract.token.1 {
                Ok(Event::GetReserves {
                    reserve_a: contract.reserve.0,
                    reserve_b: contract.reserve.1,
                    block_timestamp_last: contract.last_block_ts,
                })
            } else {
                Err(Error::InvalidTokens)
            }
        }
        InnerAction::TransferFrom { from, to, amount } => contract
            .ft_state
            .transfer_from(from, to, U256::from(amount))
            .map(Into::into)
            .map_err(Into::into),

        InnerAction::Approve { spender, amount } => contract
            .ft_state
            .approve(spender, U256::from(amount))
            .map(Into::into)
            .map_err(Into::into),
        InnerAction::BalanceOf ( owner ) => Ok(Event::Balance(contract.ft_state.balance_of(owner).low_u128())),
    }
}

#[no_mangle]
extern fn state() {
    let (Contract {
        factory,
        router,
        token,
        reserve,
        cumulative_price,
        last_block_ts,
        k_last,
        ft_state,
    }) = state_mut();

    msg::reply(
        State {
            factory: *factory,
            router: *router,
            token: *token,
            reserve: *reserve,
            cumulative_price: *cumulative_price,
            last_block_ts: *last_block_ts,
            k_last: *k_last,
            ft_state: ft_state.clone().into(),
        },
        0,
    )
    .expect("failed to encode or reply from `state()`");
}
