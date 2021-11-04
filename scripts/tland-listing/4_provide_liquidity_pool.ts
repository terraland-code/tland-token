import {isTxError, MsgExecuteContract} from '@terra-money/terra.js';
import {terra, lp_owner_wallet} from './keys';
import {Config} from "./config/config";
import {writeFileSync} from "fs";

interface ContractAddresses {
  token_address: string,
  devfund_address: string,
  advisors_address: string,
  privsale_address: string,
  pubsale_address: string,
  team_address: string,
  lp_staking_address: string,
  airdrop_address: string,
  terraswap_pair_address: string,
}

let contract_addresses: ContractAddresses = require('./files/contract_addresses.json')
let cfg: Config = require('./config/config.json');

async function ProvideLiquidity() {
  const increase_allowance = new MsgExecuteContract(
    lp_owner_wallet.key.accAddress, // sender
    contract_addresses.token_address, // contract address
    {
      increase_allowance: {
        amount: "1750000000000",
        spender: contract_addresses.terraswap_pair_address
      }
    },
    undefined
  )

  const provide_liquidity = new MsgExecuteContract(
    lp_owner_wallet.key.accAddress, // sender
    contract_addresses.terraswap_pair_address, // contract address
    {
      provide_liquidity: {
        assets: [
          {
            info: {
              token: {
                contract_addr: contract_addresses.token_address
              }
            },
            amount: "1750000000000"
          },
          {
            info: {
              native_token: {
                denom: "uusd"
              }
            },
            amount: cfg.ust_liquidity_amount
          }
        ]
      }
    }, // message
    { uusd: cfg.ust_liquidity_amount } // coins
  );

  const executeTx = await lp_owner_wallet.createAndSignTx({
    msgs: [increase_allowance, provide_liquidity]
  });

  const executeTxResult = await terra.tx.broadcast(executeTx);

  console.log(executeTxResult);

  if (isTxError(executeTxResult)) {
    throw new Error(
      `execute failed. code: ${executeTxResult.code}, codespace: ${executeTxResult.codespace}, raw_log: ${executeTxResult.raw_log}`
    );
  }
}

ProvideLiquidity()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
