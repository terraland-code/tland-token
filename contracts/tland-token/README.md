# TLAND token

This is a implementation of a TLAND token contract. It implements
the [CW20 spec](../../packages/cw20/README.md) and is designed to
be deployed as is, or imported into other contracts to easily build
cw20-compatible tokens with custom logic.

Implements:

- [x] CW20 Base
- [x] Allowances extension

## Running this contract

You will need Rust 1.44.1+ with `wasm32-unknown-unknown` target installed.

You can run unit tests on this via:

`cargo test`

Once you are happy with the content, you can compile it to wasm via:

```
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/tland_token.wasm .
ls -l tland_token.wasm
sha256sum tland_token.wasm
```

The optimized contracts are generated in the `artifacts/` directory.

## Create contract

```json
{
  "decimals": 6,
  "name": "TerraLand token",
  "symbol": "TLAND",
  "initial_balances": [
    {
      "address": "terra1mtdhy09e9j7x34jrqldsqntazlx00y6v5llf24",
      "amount": "100000000000000"
    }
  ]
}
```

