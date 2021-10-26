import {LCDClient, MnemonicKey, MsgInstantiateContract, isTxError} from '@terra-money/terra.js';
import {NetworkConfig} from "./config/network/config";
import {InstantiateConfig} from "./config/instantiate/config";

let networkConfig: NetworkConfig = require('./config/network/config.json');
let config: InstantiateConfig = require('./config/instantiate/config.json');

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

async function Deploy() {
    const instantiate = new MsgInstantiateContract(
        wallet.key.accAddress, // sender
        undefined, // admin
        config.codeID,
        config.msg, // InitMsg
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
}

Deploy()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
