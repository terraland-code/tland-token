import {LCDClient, MnemonicKey, MsgMigrateContract} from '@terra-money/terra.js';
import {NetworkConfig} from "./config/network/config";
import {MigrateConfig} from "./config/migrate/config";

let networkConfig: NetworkConfig = require('./config/network/config.json');
let config: MigrateConfig = require('./config/migrate/config.json');

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

async function Migrate() {
  const migrate = new MsgMigrateContract(
    wallet.key.accAddress, // sender
    config.contractAddress,
    config.codeID,
    config.msg // InitMsg
  );

  console.log(wallet.key.accAddress)

  const migrateTx = await wallet.createAndSignTx({
    msgs: [migrate],
  });
  const migrateTxResult = await terra.tx.broadcast(migrateTx);

  console.log(migrateTxResult);
}

Migrate()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
