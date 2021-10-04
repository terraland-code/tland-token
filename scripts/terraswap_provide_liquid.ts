import {LCDClient, MnemonicKey, isTxError, MsgExecuteContract} from '@terra-money/terra.js';
import { config } from './config/config';

const token_address = "terra1qqnpne27ew4mm3fqt6sqjkjhsy724pyphy8qzf";
const terraswap_pool_address = "terra1femgl9m3glq9qheryr40ca7ekn73n87ktegjfj";

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
    const increase_allowance = new MsgExecuteContract(
        wallet.key.accAddress, // sender
        token_address, // contract address
        {
            increase_allowance: {
                amount: "100000000000",
                spender: terraswap_pool_address
            }
        },
        undefined
    )

    const provide_liquidity = new MsgExecuteContract(
        wallet.key.accAddress, // sender
        terraswap_pool_address, // contract address
        {
            provide_liquidity: {
                assets: [
                    {
                        info: {
                            token: {
                                contract_addr: token_address
                            }
                        },
                        amount: "100000000000"
                    },
                    {
                        info: {
                            native_token: {
                                denom: "uusd"
                            }
                        },
                        amount: "1000000000"
                    }
                ]
            }
        }, // message
        { uusd: 1000000000 } // coins
    );

    const executeTx = await wallet.createAndSignTx({
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

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
