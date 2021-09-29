import {LCDClient, MnemonicKey, MsgInstantiateContract, isTxError} from '@terra-money/terra.js';
import { config } from './config/config';

const token_code_id = 9795;

// create a key out of a mnemonic
const mk = new MnemonicKey({
    mnemonic: config.mnemonic,
});

// connect to tequila testnet
const terra = new LCDClient({
    URL: 'https://tequila-lcd.terra.dev',
    chainID: 'tequila-0004'
});

const wallet = terra.wallet(mk);

async function main() {
    const instantiate = new MsgInstantiateContract(
        wallet.key.accAddress, // owner
        token_code_id,
        {
            decimals: 8,
            name: 'FCQplatform.com native token',
            symbol: 'FCQN',
            initial_balances: [
                {
                    address: 'terra1mtdhy09e9j7x34jrqldsqntazlx00y6v5llf24',
                    amount: '10000000000000000'
                }
            ]
        }, // InitMsg
        undefined, // init coins
        true, // migratable
        undefined, // sender
        undefined // admin
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

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
