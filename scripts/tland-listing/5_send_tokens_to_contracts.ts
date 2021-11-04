import { isTxError, MsgExecuteContract, Wallet } from "@terra-money/terra.js";
import {
  advisors_owner_wallet, airdrop_owner_wallet, delay,
  devfund_owner_wallet,
  lp_owner_wallet,
  privsale_owner_wallet,
  pubsale_owner_wallet,
  staking_owner_wallet,
  team_owner_wallet,
  terra
} from "./keys";
import {Config} from "./config/config";

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

async function SendTokens(wallet: Wallet, recipient: string, amount: string) {
  const transfer = new MsgExecuteContract(
    wallet.key.accAddress, // sender
    contract_addresses.token_address, // contract address
    {
      transfer: {
        amount: amount,
        recipient: recipient
      }
    },
    undefined
  )

  const executeTx = await wallet.createAndSignTx({
    msgs: [transfer]
  });

  const executeTxResult = await terra.tx.broadcast(executeTx);

  console.log(executeTxResult);

  if (isTxError(executeTxResult)) {
    throw new Error(
      `execute failed. code: ${executeTxResult.code}, codespace: ${executeTxResult.codespace}, raw_log: ${executeTxResult.raw_log}`
    );
  }
}

async function SendAllTokens() {
  // Team smart contract 17 mln
  await SendTokens(team_owner_wallet, contract_addresses.team_address, "17000000000000")
  delay(10000)
  // Staking 13 mln
  await SendTokens(staking_owner_wallet, contract_addresses.lp_staking_address, "12000000000000")
  delay(10000)
  // Priv sale 20 mln
  await SendTokens(privsale_owner_wallet, contract_addresses.privsale_address,"20000000000000")
  delay(10000)
  // Pub sale 1111111,120000
  await SendTokens(pubsale_owner_wallet, contract_addresses.pubsale_address, "1111111120000")
  delay(10000)
  // Development fund 5 mln
  await SendTokens(devfund_owner_wallet, contract_addresses.devfund_address,"5000000000000" )
  delay(10000)
  // Advisors 4 mln
  await SendTokens(advisors_owner_wallet, contract_addresses.advisors_address, "4000000000000")
  delay(10000)
  // Airdrop 1 mln
  await SendTokens(airdrop_owner_wallet, contract_addresses.airdrop_address, "1000000000000")
}

SendAllTokens()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });

