import {isTxError, MsgInstantiateContract, Wallet} from '@terra-money/terra.js';
import {terra, token_owner_wallet} from './keys';
import {writeFileSync} from "fs";

interface CodeIds {
  token_code_id: string,
  staking_code_id: string,
  airdrop_code_id: string,
  vesting_code_id: string,
}

let code_ids: CodeIds = require('./files/code_ids.json');

async function Instantiate(wallet: Wallet, code_id: number, msg: object) {
  const instantiate = new MsgInstantiateContract(
    wallet.key.accAddress, // sender
    wallet.key.accAddress, // admin
    code_id,
    msg, // InitMsg
    undefined, // init coins
  );

  const instantiateTx = await wallet.createAndSignTx({
    msgs: [instantiate],
  });
  const instantiateTxResult = await terra.tx.broadcast(instantiateTx);

  console.log(instantiateTxResult);

  if (isTxError(instantiateTxResult)) {
    throw new Error(
      `instantiate failed. code: ${instantiateTxResult.code}, codespace: ${instantiateTxResult.codespace}, raw_log: ${instantiateTxResult.raw_log}`
    );
  }

  const {instantiate_contract: {contract_address}} = instantiateTxResult.logs[0].eventsByType;
  console.log(`contract_address: ${contract_address}`)

  return contract_address
}

async function DeployContracts() {
  let token_address = Instantiate(token_owner_wallet, parseInt(code_ids.token_code_id),
    {
      decimals: 6,
      name: "TerraLand token",
      symbol: "TLAND",
      initial_balances: [
        // Treasury 25 000 000 $
        {
          address: "terra1ly5glvd0xv5x5s4vd5x6a8p8n4pcmwn839pcep",
          amount: "25000000000000"
        },
        // Team 17 000 000 $
        {
          address: "terra1hek3fzkmke5pe6lvcv48frwvchc04fz6y22fyj",
          amount: "17000000000000"
        },
        // Community incentives, Staking 13 000 000 $
        {
          address: "terra1yjwlg6dy3dkq3qlhv0feyqe7wrt3q0h0ghlwcy",
          amount: "13000000000000"
        },
        // Private sale & marketing partnerships 20 000 000 $
        {
          address: "terra1ksawlatvhqmm3lg9uc7w6z20zvvuegmwxgjtpm",
          amount: "20000000000000"
        },
        // Public sale 10 000 000 $
        {
          address: "terra1wcuvasqx8zf69e9jhnnxgk4em7dnemqappugwj",
          amount: "10000000000000"
        },
        // Development fund 5 000 000 $
        {
          address: "terra1z43ptner54dvkpz2cuyu67wjkzmqzs2kq8wsu5",
          amount: "5000000000000",
        },
        // Advisors fund 4 000 000 $
        {
          address: "terra1u4cukjhadget74ugc4antvca0v0jzlxxmjp5t7",
          amount: "4000000000000",
        },
        // Liquidity for smart contract 1 750 000 $
        {
          address: "terra1w6402sdfcu4smfhunqvwyv6kwq87f2kvnc4z0m",
          amount: "1750000000000",
        },
        // Liquidity rest 3 250 000 $
        {
          address: "terra1w6402sdfcu4smfhunqvwyv6kwq87f2kvnc4z0m",
          amount: "3250000000000",
        },
        // Airdrop 1 000 000 $
        {
          address: "terra1amskskeaput62xdahpp59yrw4fm7g7ndgsc20u",
          amount: "1000000000000",
        }
      ]
    }
  )

  let contract_addresses = {
    token_address: token_address,
  }

  let jsonData = JSON.stringify(contract_addresses);
  writeFileSync("files/contract_addresses.json", jsonData);
}

DeployContracts()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
