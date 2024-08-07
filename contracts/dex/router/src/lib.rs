#![no_std]

use dex_factory_io::{Action as FactoryAction, Error as FactoryError, Event as FactoryEvent};
use dex_io::{Action as PairAction, Error as PairError, Event as PairEvent};
use dex_router_io::router_utils::*;
use dex_router_io::*;
use dex_wvara_io::{Action as WVARAAction, Error as WVARAError, Event as WVARAEvent};
use gstd::{
    collections::HashMap,
    errors::Result,
    exec::{self, program_id},
    msg::{self, value},
    prelude::*,
    prog::ProgramGenerator,
    util, ActorId, CodeId, MessageId,
};
struct Contract {
    pub factory: ActorId,
    pub wvara: ActorId,
}

static mut STATE: Option<Contract> = None;

impl Contract {
    async fn add_liquidity(
        &self,
        token_a: ActorId,
        token_b: ActorId,
        amount_a_desired: u128,
        amount_b_desired: u128,
        amount_a_min: u128,
        amount_b_min: u128,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        // check_deadline(deadline)?;

        let pair_res: Result<FactoryEvent, FactoryError> =
            send(self.factory, FactoryAction::GetPair(token_a, token_b), 0)?.await?;
        let pair_id: ActorId;
        if let Ok(FactoryEvent::Pair(pair)) = pair_res {
            pair_id = pair;
        } else {
            // create pair
            let pair_res: Result<FactoryEvent, FactoryError> = send(
                self.factory,
                FactoryAction::CreatePair(token_a, token_b, program_id()),
                0,
            )?
            .await?;
            let Ok(FactoryEvent::PairCreated {
                token_pair,
                pair_actor,
                pair_number,
            }) = pair_res
            else {
                return Err(Error::PairCreationFailed);
            };
            pair_id = pair_actor;
        }

        let mut amount_desired = (amount_a_desired, amount_b_desired);
        let mut amount_min = (amount_a_min, amount_b_min);
        let mut token = (token_a, token_b);

        if (token != sort_tokens(token_a, token_b)) {
            amount_desired = (amount_b_desired, amount_a_desired);
            amount_min = (amount_b_min, amount_a_min);
            token = (token_b, token_a);
        }

        let add_liquidity_res: Result<PairEvent, PairError> = send(
            pair_id,
            PairAction::AddLiquidity {
                amount_a_desired: amount_desired.0,
                amount_b_desired: amount_desired.1,
                amount_a_min: amount_min.0,
                amount_b_min: amount_min.1,
                to,
                deadline,
            },
            0,
        )?
        .await?;
        let Ok(PairEvent::AddedLiquidity {
            amount_a,
            amount_b,
            liquidity,
            sender,
        }) = add_liquidity_res
        else {
            return Err(Error::AddLiquidityFailed);
        };

        // transfer tokens to pair
        transfer_tokens(token.0, msg::source(), pair_id, amount_a).await?;
        transfer_tokens(token.1, msg::source(), pair_id, amount_b).await?;

        Ok(Event::AddLiquidity {
            token_a: token.0,
            token_b: token.1,
            liquidity: liquidity.as_u128(),
            amount_a,
            amount_b,
        })
    }
    async fn add_liquidity_vara(
        &self,
        token: ActorId,
        amount_token_desired: u128,
        amount_token_min: u128,
        amount_vara_min: u128,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        // check_deadline(deadline)?;

        let pair_res: Result<FactoryEvent, FactoryError> =
            send(self.factory, FactoryAction::GetPair(token, self.wvara), 0)?.await?;
        let pair_id: ActorId;
        if let Ok(FactoryEvent::Pair(pair)) = pair_res {
            pair_id = pair;
        } else {
            // create pair
            let pair_res: Result<FactoryEvent, FactoryError> = send(
                self.factory,
                FactoryAction::CreatePair(token, self.wvara, program_id()),
                0,
            )?
            .await?;
            let Ok(FactoryEvent::PairCreated {
                token_pair,
                pair_actor,
                pair_number,
            }) = pair_res
            else {
                // refund vara if pair creation failed
                send(msg::source(), {}, msg::value())?.await?;
                return Err(Error::PairCreationFailed);
            };
            pair_id = pair_actor;
        }

        let mut amount_desired = (amount_token_desired, msg::value());
        let mut amount_min = (amount_token_min, amount_vara_min);
        let mut token_sort = (token, self.wvara);

        if (token_sort != sort_tokens(token, self.wvara)) {
            amount_desired = (msg::value(), amount_token_desired);
            amount_min = (amount_vara_min, amount_token_min);
            token_sort = (self.wvara, token);
        }

        let add_liquidity_res: Result<PairEvent, PairError> = send(
            pair_id,
            PairAction::AddLiquidity {
                amount_a_desired: amount_desired.0,
                amount_b_desired: amount_desired.1,
                amount_a_min: amount_min.0,
                amount_b_min: amount_min.1,
                to,
                deadline,
            },
            0,
        )?
        .await?;
        let Ok(PairEvent::AddedLiquidity {
            amount_a,
            amount_b,
            liquidity,
            sender,
        }) = add_liquidity_res
        else {
            // refund vara if add liquidity failed
            send(msg::source(), {}, msg::value())?.await?;
            return Err(Error::AddLiquidityFailed);
        };

        // transfer tokens to pair
        transfer_tokens(token, msg::source(), pair_id, amount_token_desired).await?;

        let amount_vara = if token_sort.0 == self.wvara {
            amount_a
        } else {
            amount_b
        };

        // transfer vara to pair
        //1. deposit vara to wvara
        //2. transfer wvara to pair
        let deposit_vara_re: Result<WVARAEvent, WVARAError> =
            send(self.wvara, WVARAAction::Deposit, amount_vara)?.await?;
        let Ok(WVARAEvent::Deposit { from, amount }) = deposit_vara_re else {
            // refund vara if deposit failed
            send(msg::source(), {}, msg::value())?.await?;
            return Err(Error::DepositVaraFailed);
        };
        let transfer_wvara_res: Result<WVARAEvent, WVARAError> = send(
            self.wvara,
            WVARAAction::Transfer {
                from: program_id(),
                to: pair_id,
                amount: amount_vara,
            },
            0,
        )?
        .await?;
        let Ok(WVARAEvent::Transfer { from, to, amount }) = transfer_wvara_res else {
            return Err(Error::TransferFailed);
        };

        if msg::value() > amount_vara {
            // refund
            send(msg::source(), {}, msg::value() - amount_vara)?.await?;
        }
        Ok(Event::AddLiquidity {
            token_a: token_sort.0,
            token_b: token_sort.1,
            liquidity: liquidity.as_u128(),
            amount_a,
            amount_b,
        })
    }
    async fn remove_liquidity(
        &self,
        token_a: ActorId,
        token_b: ActorId,
        liquidity: u128,
        amount_a_min: u128,
        amount_b_min: u128,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        // check_deadline(deadline)?;

        let pair_res: Result<FactoryEvent, FactoryError> =
            send(self.factory, FactoryAction::GetPair(token_a, token_b), 0)?.await?;
        let pair_id: ActorId;
        if let Ok(FactoryEvent::Pair(pair)) = pair_res {
            pair_id = pair;
        } else {
            return Err(Error::PairNotFound);
        }

        // transfer liquidity to router
        let send_liquidity_res: Result<PairEvent, PairError> = send(
            pair_id,
            PairAction::TransferFrom {
                from: msg::source(),
                to: program_id(),
                amount: liquidity,
            },
            0,
        )?
        .await?;

        let Err(err) = send_liquidity_res else {
            return Err(Error::TransferFailed);
        };

        let pair_res: Result<PairEvent, PairError> = send(
            pair_id,
            PairAction::RemoveLiquidity {
                liquidity: liquidity.into(),
                amount_a_min,
                amount_b_min,
                to,
                deadline,
            },
            0,
        )?
        .await?;
        let Ok(PairEvent::RemovedLiquidity {
            amount_a,
            amount_b,
            sender,
            to,
        }) = pair_res
        else {
            return Err(Error::RemovedLiquidityFailed);
        };

        Ok(Event::RemoveLiquidity {
            token_a,
            token_b,
            liquidity,
            amount_a,
            amount_b,
        })
    }

    async fn remove_liquidity_vara(
        &self,
        token: ActorId,
        liquidity: u128,
        amount_token_min: u128,
        amount_vara_min: u128,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        // check_deadline(deadline)?;

        let pair_res: Result<FactoryEvent, FactoryError> =
            send(self.factory, FactoryAction::GetPair(token, self.wvara), 0)?.await?;
        let pair_id: ActorId;
        if let Ok(FactoryEvent::Pair(pair)) = pair_res {
            pair_id = pair;
        } else {
            return Err(Error::PairNotFound);
        }

        // transfer liquidity to router
        let send_liquidity_res: Result<PairEvent, PairError> = send(
            pair_id,
            PairAction::TransferFrom {
                from: msg::source(),
                to: program_id(),
                amount: liquidity,
            },
            0,
        )?
        .await?;

        let Err(err) = send_liquidity_res else {
            return Err(Error::TransferFailed);
        };

        let mut token_sort = sort_tokens(token, self.wvara);
        let mut amount_min = (amount_token_min, amount_vara_min);

        if token_sort != (token, self.wvara) {
            amount_min = (amount_vara_min, amount_token_min);
        };

        let pair_res: Result<PairEvent, PairError> = send(
            pair_id,
            PairAction::RemoveLiquidity {
                liquidity: liquidity.into(),
                amount_a_min: amount_min.0,
                amount_b_min: amount_min.1,
                to:program_id(),
                deadline,
            },
            0,
        )?
        .await?;
        let Ok(PairEvent::RemovedLiquidity {
            amount_a,
            amount_b,
            sender,
            to,
        }) = pair_res
        else {
            return Err(Error::RemovedLiquidityFailed);
        };

        if token == token_sort.0 {
            // transfer token to to
            transfer_tokens(token, program_id(), to, amount_a).await?;
            // withdraw vara to to
            let withdraw_vara_res: Result<WVARAEvent, WVARAError> = send(
                self.wvara,
                WVARAAction::Withdraw {
                    to,
                    amount: amount_b,
                },
                0,
            )?.await?;
            let Ok(WVARAEvent::Withdraw { amount, to }) = withdraw_vara_res else {
                return Err(Error::WithdrawVaraFailed);
            };
        }else {
            // transfer token to to
            transfer_tokens(token, program_id(), to, amount_b).await?;
            // withdraw vara to to
            let withdraw_vara_res: Result<WVARAEvent, WVARAError> = send(
                self.wvara,
                WVARAAction::Withdraw {
                    to,
                    amount: amount_a,
                },
                0,
            )?.await?;
            let Ok(WVARAEvent::Withdraw { amount, to }) = withdraw_vara_res else {
                return Err(Error::WithdrawVaraFailed);
            };
        }
        

        Ok(Event::RemoveLiquidity {
            token_a: token_sort.0,
            token_b: token_sort.1,
            liquidity,
            amount_a,
            amount_b,
        })
    }

    async fn internal_swap(
        &self,
        amounts: Vec<u128>,
        path: Vec<ActorId>,
        _to: ActorId,
    ) -> Result<(), Error> {
        for i in 0..path.len() - 1 {
            let pair_res: Result<FactoryEvent, FactoryError> = send(
                self.factory,
                FactoryAction::GetPair(path[i], path[i + 1]),
                0,
            )?
            .await?;
            let pair_id: ActorId;
            if let Ok(FactoryEvent::Pair(pair)) = pair_res {
                pair_id = pair;
            } else {
                return Err(Error::PairNotFound);
            }

            let (in_put, out_put) = (path[i], path[i + 1]);
            let (token_0, _) = sort_tokens(in_put, out_put);

            let mut amount_in = amounts[i];
            let mut amount_out = amounts[i + 1];
            let mut a_to_b = true;

            if in_put != token_0 {
                amount_in = amount_out;
                amount_out = amount_in;
                a_to_b = false;
            }

            let to = if i < path.len() - 2 {
                let pair_2_res: Result<FactoryEvent, FactoryError> = send(
                    self.factory,
                    FactoryAction::GetPair(out_put, path[i + 2]),
                    0,
                )?
                .await?;
                if let Ok(FactoryEvent::Pair(pair)) = pair_2_res {
                    pair
                } else {
                    return Err(Error::PairNotFound);
                }
            } else {
                _to
            };

            let swap_res: Result<PairEvent, PairError> = send(
                pair_id,
                PairAction::Swap {
                    in_amount: amount_in,
                    out_amount: amount_out,
                    to,
                    a_to_b,
                },
                0,
            )?
            .await?;
            if let Err(err) = swap_res {
                return Err(Error::LPPairError(err));
            };
        }
        Ok(())
    }

    async fn swap_exact_tokens_for_tokens(
        &self,
        amount_in: u128,
        amount_out_min: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        check_deadline(deadline)?;
        let amounts = get_amounts_out(self.factory, amount_in, path.clone()).await?;
        if amounts[amounts.len() - 1] < amount_out_min {
            return Err(Error::InsufficientOutputAmount);
        }
        let pair = get_pair(self.factory, path[0], path[1]).await?;
        //transfer token to pair
        transfer_tokens(path[0], msg::source(), pair, amounts[0]).await?;
        self.internal_swap(amounts.clone(), path.clone(), to)
            .await?;
        Ok(Event::SwapExactTokensForTokens {
            amount_in,
            path,
            amount_out: amounts[amounts.len() - 1],
            amounts,
        })
    }

    async fn swap_tokens_for_exact_tokens(
        &self,
        amount_out: u128,
        amount_in_max: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        check_deadline(deadline)?;
        let amounts = get_amounts_in(self.factory, amount_out, path.clone()).await?;
        if amounts[0] > amount_in_max {
            return Err(Error::ExcessiveInputAmount);
        }
        let pair = get_pair(self.factory, path[0], path[1]).await?;
        //transfer token to pair
        transfer_tokens(path[0], msg::source(), pair, amounts[0]).await?;
        self.internal_swap(amounts.clone(), path.clone(), to).await?;
        Ok(Event::SwapTokensForExactTokens {
            amount_out,
            path,
            amount_in: amounts[0],
            amounts,
        })
    }
    async fn swap_exact_vara_for_tokens(
        &self,
        amount_out_min: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        check_deadline(deadline)?;
        if path[0] != self.wvara {
            return Err(Error::InvalidPath);
        };

        let amounts = get_amounts_out(self.factory, msg::value(), path.clone()).await?;
        if amounts[amounts.len() - 1] < amount_out_min {
            return Err(Error::InsufficientOutputAmount);
        }
        let pair = get_pair(self.factory, path[0], path[1]).await?;
        //deposit vara to wvara
        let deposit_vara_res: Result<WVARAEvent, WVARAError> =
            send(self.wvara, WVARAAction::Deposit, amounts[0])?.await?;
        let Ok(WVARAEvent::Deposit { from, amount }) = deposit_vara_res else {
            return Err(Error::DepositVaraFailed);
        };
        //transfer wvara to pair
        let transfer_wvara_res: Result<WVARAEvent, WVARAError> = send(
            self.wvara,
            WVARAAction::Transfer {
                from: program_id(),
                to: pair,
                amount: amounts[0],
            },
            0,
        )?
        .await?;
        let Ok(WVARAEvent::Transfer { from, to, amount }) = transfer_wvara_res else {
            return Err(Error::TransferFailed);
        };
        self.internal_swap(amounts.clone(), path.clone(), to).await?;
        Ok(Event::SwapExactVARAForTokens {
            path,
            amount_out: amounts[amounts.len() - 1],
            amount_in: amounts[0],
            amounts,
            
        })
    }

    async fn swap_tokens_for_exact_vara(
        &self,
        amount_out: u128,
        amount_in_max: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        check_deadline(deadline)?;
        if path[path.len() - 1] != self.wvara {
            return Err(Error::InvalidPath);
        };
        let amounts = get_amounts_in(self.factory, amount_out, path.clone()).await?;
        if amounts[0] > amount_in_max {
            return Err(Error::ExcessiveInputAmount);
        }
        let pair = get_pair(self.factory, path[0], path[1]).await?;
        //transfer token to pair
        transfer_tokens(path[0], msg::source(), pair, amounts[0]).await?;
        self.internal_swap(amounts.clone(), path.clone(), to).await?;
        //withdraw vara from wvara to msg::source()
        let withdraw_vara_res: Result<WVARAEvent, WVARAError> = send(
            self.wvara,
            WVARAAction::Withdraw {
                to,
                amount: amounts[amounts.len() - 1],
            },
            0,
        )?
        .await?;

        let Ok(WVARAEvent::Withdraw { amount, to }) = withdraw_vara_res else {
            return Err(Error::WithdrawVaraFailed);
        };

        Ok(Event::SwapTokensForExactVARA {
            amount_out: amounts[amounts.len() - 1],
            path,
            amount_in: amounts[0],
            amounts,
        })
    }

    async fn swap_exact_tokens_for_vara(
        &self,
        amount_in: u128,
        amount_out_min: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        check_deadline(deadline)?;
        if path[path.len() - 1] != self.wvara {
            return Err(Error::InvalidPath);
        };
        let amounts = get_amounts_out(self.factory, amount_in, path.clone()).await?;
        if amounts[amounts.len() - 1] < amount_out_min {
            return Err(Error::InsufficientOutputAmount);
        }
        let pair = get_pair(self.factory, path[0], path[1]).await?;
        //transfer token to pair
        transfer_tokens(path[0], msg::source(), pair, amounts[0]).await?;
        self.internal_swap(amounts.clone(), path.clone(), to).await?;
        //withdraw vara from wvara to msg::source()
        let withdraw_vara_res: Result<WVARAEvent, WVARAError> = send(
            self.wvara,
            WVARAAction::Withdraw {
                to,
                amount: amounts[amounts.len() - 1],
            },
            0,
        )?
        .await?;

        let Ok(WVARAEvent::Withdraw { amount, to }) = withdraw_vara_res else {
            return Err(Error::WithdrawVaraFailed);
        };

        Ok(Event::SwapExactTokensForVARA {
            amount_in,
            path,
            amount_out: amounts[amounts.len() - 1],
            amounts,
        })
    }
    async fn swap_vara_for_exact_tokens(
        &self,
        amount_out: u128,
        path: Vec<ActorId>,
        to: ActorId,
        deadline: u64,
    ) -> Result<Event, Error> {
        check_deadline(deadline)?;
        if path[0] != self.wvara {
            return Err(Error::InvalidPath);
        };

        let amounts = get_amounts_in(self.factory, amount_out, path.clone()).await?;
        if msg::value() < amounts[0] {
            return Err(Error::ExcessiveInputAmount);
        }
        let pair = get_pair(self.factory, path[0], path[1]).await?;
        //deposit vara to wvara
        let deposit_vara_res: Result<WVARAEvent, WVARAError> =
            send(self.wvara, WVARAAction::Deposit, amounts[0])?.await?;
        let Ok(WVARAEvent::Deposit { from, amount }) = deposit_vara_res else {
            return Err(Error::DepositVaraFailed);
        };
        //transfer wvara to pair
        let transfer_wvara_res: Result<WVARAEvent, WVARAError> = send(
            self.wvara,
            WVARAAction::Transfer {
                from: program_id(),
                to: pair,
                amount: amounts[0],
            },
            0,
        )?
        .await?;
        let Ok(WVARAEvent::Transfer { from, to, amount }) = transfer_wvara_res else {
            return Err(Error::TransferFailed);
        };
        self.internal_swap(amounts.clone(), path.clone(), to).await?;
        //refund dust vara to msg::source() if any
        if msg::value() > amounts[0] {
            send(msg::source(), {}, msg::value() - amounts[0])?.await?;
        }

        Ok(Event::SwapVARAForExactTokens {
            amount_out,
            amounts:amounts.clone(),
            amount_in: amounts[0],
            path,
        })
    }
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

    reply(result).expect("failed to encode or reply from `init()`");

    if is_err {
        exec::exit(ActorId::zero());
    }
}

fn process_init() -> Result<(), Error> {
    let Initialize { factory, wvara } = msg::load()?;

    unsafe {
        STATE = Some(Contract { factory, wvara });
    };

    Ok(())
}

#[gstd::async_main]
async fn main() {
    reply(process_handle().await).expect("failed to encode or reply `handle()`");
}

async fn process_handle() -> Result<Event, Error> {
    let action: Action = msg::load()?;
    let contract = state_mut();

    match action {
        Action::AddLiquidity {
            token_a,
            token_b,
            amount_a_desired,
            amount_b_desired,
            amount_a_min,
            amount_b_min,
            to,
            deadline,
        } => {
            contract
                .add_liquidity(
                    token_a,
                    token_b,
                    amount_a_desired,
                    amount_b_desired,
                    amount_a_min,
                    amount_b_min,
                    to,
                    deadline,
                )
                .await
        }
        Action::AddLiquidityVARA {
            token,
            amount_token_desired,
            amount_token_min,
            amount_vara_min,
            to,
            deadline,
        } => {
            contract
                .add_liquidity_vara(
                    token,
                    amount_token_desired,
                    amount_token_min,
                    amount_vara_min,
                    to,
                    deadline,
                )
                .await
        }
        Action::RemoveLiquidity {
            token_a,
            token_b,
            liquidity,
            amount_a_min,
            amount_b_min,
            to,
            deadline,
        } => {
            contract
                .remove_liquidity(
                    token_a,
                    token_b,
                    liquidity,
                    amount_a_min,
                    amount_b_min,
                    to,
                    deadline,
                )
                .await
        }
        Action::RemoveLiquidityVARA {
            token,
            liquidity,
            amount_token_min,
            amount_vara_min,
            to,
            deadline,
        } => {
            contract
                .remove_liquidity_vara(
                    token,
                    liquidity,
                    amount_token_min,
                    amount_vara_min,
                    to,
                    deadline,
                )
                .await
        }
        Action::SwapExactTokensForTokens {
            amount_in,
            amount_out_min,
            path,
            to,
            deadline,
        } => {
            contract
                .swap_exact_tokens_for_tokens(amount_in, amount_out_min, path, to, deadline)
                .await
        }
        Action::SwapTokensForExactTokens {
            amount_out,
            amount_in_max,
            path,
            to,
            deadline,
        } => {
            contract
                .swap_tokens_for_exact_tokens(amount_out, amount_in_max, path, to, deadline)
                .await
        }
        Action::SwapExactVARAForTokens {
            amount_out_min,
            path,
            to,
            deadline,
        } => {
            contract
                .swap_exact_vara_for_tokens(amount_out_min, path, to, deadline)
                .await
        }
        Action::SwapExactTokensForVARA {
            amount_in,
            amount_out_min,
            path,
            to,
            deadline,
        } => {
            contract
                .swap_exact_tokens_for_vara(amount_in, amount_out_min, path, to, deadline)
                .await
        }
        Action::SwapTokensForExactVARA {
            amount_out,
            amount_in_max,
            path,
            to,
            deadline,
        } => {
            contract
                .swap_tokens_for_exact_vara(amount_out, amount_in_max, path, to, deadline)
                .await
        }
        Action::SwapVARAForExactTokens {
            amount_out,
            path,
            to,
            deadline,
        } => {
            contract
                .swap_vara_for_exact_tokens(amount_out, path, to, deadline)
                .await
        }
    }
}

fn state_mut() -> &'static mut Contract {
    unsafe { STATE.as_mut().expect("state isn't initialized") }
}

#[no_mangle]
extern fn state() {
    let Contract { factory, wvara } = state_mut();

    reply(State {
        factory: *factory,
        wvara: *wvara,
    })
    .expect("failed to encode or reply from `state()`");
}

fn reply(payload: impl Encode) -> Result<MessageId> {
    Ok(msg::reply(payload, 0)?)
}
