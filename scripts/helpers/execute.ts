import {LCDClient, MnemonicKey, MsgExecuteContract} from '@terra-money/terra.js';
import {NetworkConfig} from "./config/network/config";
import {ExecuteConfig} from "./config/execute/config";

let networkConfig: NetworkConfig = require('./config/network/config.json');
let config: ExecuteConfig = require('./config/execute/airdrop/register_members.config.json');

// create a key out of a mnemonic
const mk = new MnemonicKey({
  mnemonic: networkConfig.mnemonic,
});

// connect to tequila testnet
const terra = new LCDClient({
  URL: networkConfig.url,
  chainID: networkConfig.chainID,
});

const wallet = terra.wallet(mk);

async function Execute() {
  const execute = new MsgExecuteContract(
    wallet.key.accAddress, // sender
    config.contractAddress, // contract address
    config.msg, // message
    config.coins, // coins
  );

  const executeTx = await wallet.createAndSignTx({
    msgs: [execute]
  });

  const executeTxResult = await terra.tx.broadcast(executeTx);

  console.log(executeTxResult);
}

Execute()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
