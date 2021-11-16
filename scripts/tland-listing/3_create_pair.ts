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

async function CreatePair() {
  const execute = new MsgExecuteContract(
    lp_owner_wallet.key.accAddress, // sender
    cfg.terraswap_factory_address, // contract address
    {
      create_pair: {
        asset_infos: [
          {
            token: {
              contract_addr: contract_addresses.token_address
            }
          },
          {
            native_token: {
              denom: "uusd"
            }
          }
        ]
      }
    }, // message
    undefined // coins
  );

  const executeTx = await lp_owner_wallet.createAndSignTx({
    msgs: [execute]
  });

  const executeTxResult = await terra.tx.broadcast(executeTx);

  console.log(executeTxResult);

  if (isTxError(executeTxResult)) {
    throw new Error(
      `execute failed. code: ${executeTxResult.code}, codespace: ${executeTxResult.codespace}, raw_log: ${executeTxResult.raw_log}`
    );
  }

  const {wasm: {pair_contract_addr}} = executeTxResult.logs[0].eventsByType;
  console.log(`pair_contract_address: ${pair_contract_addr}`)

  // add terraswap_pair_address
  contract_addresses.terraswap_pair_address = pair_contract_addr[0]
  let jsonData = JSON.stringify(contract_addresses);
  writeFileSync("files/contract_addresses.json", jsonData);
}

CreatePair()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
