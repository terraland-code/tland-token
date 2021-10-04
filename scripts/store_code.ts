import {isTxError, LCDClient, MnemonicKey, MsgStoreCode} from '@terra-money/terra.js';
import { readFileSync } from 'fs';
import { config } from './config/config';

const code_path = '../artifacts/fcqn.wasm';

// create a key out of a mnemonic
const mk = new MnemonicKey({
    mnemonic: config.mnemonic,
});

// connect to tequila testnet
const terra = new LCDClient({
    URL: 'https://bombay-lcd.terra.dev',
    chainID: 'bombay-12',
});

const wallet = terra.wallet(mk);

async function main() {
    const storeCode = new MsgStoreCode(
        wallet.key.accAddress,
        readFileSync(code_path).toString('base64')
    );
    const storeCodeTx = await wallet.createAndSignTx({
        msgs: [storeCode],
    });
    const storeCodeTxResult = await terra.tx.broadcast(storeCodeTx);

    console.log(storeCodeTxResult);

    if (isTxError(storeCodeTxResult)) {
        throw new Error(
            `store code failed. code: ${storeCodeTxResult.code}, codespace: ${storeCodeTxResult.codespace}, raw_log: ${storeCodeTxResult.raw_log}`
        );
    }

    const {store_code: {code_id}} = storeCodeTxResult.logs[0].eventsByType;
    console.log(`code_id: ${code_id}`);
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });