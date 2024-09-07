# Hoku Registrar

[![License](https://img.shields.io/github/license/hokunet/registrar.svg)](./LICENSE)
[![standard-readme compliant](https://img.shields.io/badge/standard--readme-OK-green.svg)](https://github.com/RichardLitt/standard-readme)

> Account registration service for Hoku

## Background

This web-based service facilitates the creation of new Hoku accounts.
It triggers the FEVM to create accounts by calling
the [Hoku Faucet](https://github.com/hokunet/contracts/blob/main/src/Faucet.sol)'s `drip` method from a service wallet.

## Usage

```sh
curl -X POST -H 'Content-Type: application/json' 'http://<LISTEN_HOST:LISTEN_HOST>/register' --data-raw '{"address": "0xfoobar"}'
```

```json
{
  "tx_hash": "0x4118b732581c3ab9134b2619434197323c5d55c591611e98206645ba84a4b75e"
}
```

Use `"wait": false` to return the transaction hash immediately, without waiting for confirmation. By default, the
request waits for confirmation.

```sh
curl -X POST -H 'Content-Type: application/json' 'http://<LISTEN_HOST>:<LISTEN_HOST>/register' --data-raw '{"address": "0xfoobar", "wait": false}'
```

### Errors

#### 400 Bad Request

```json
{
  "code": 400,
  "message": "<error detail>"
}
```

#### 429 Too Many Requests

```json
{
  "code": 429,
  "message": "too many requests"
}
```

#### 503 Service Unavailable

```json
{
  "code": 503,
  "message": "faucet empty"
}
```

## Development

### Build docker image

```sh
make build
```

### Run the service

- `PRIVATE_KEY`: A private key from any wallet that exists on the Hoku chain and has non-zero `HOKU` balance.
- `FAUCET_ADDRESS`: The contract address of
  a [Hoku Faucet](https://github.com/hokunet/contracts/blob/main/src/Faucet.sol).
- `EVM_RPC_URL`: An Ethereum RPC URL of a Hoku validator. The default is `http://127.0.0.1:8545`.
- `LISTEN_HOST`: The host that the service will bind to. The defualt is `127.0.0.1`.
- `LISTEN_PORT`: The port that the service will bind to. The default is `8080`.

```sh
PRIVATE_KEY=<> FAUCET_ADDRESS=<> make run
```

For local testing, use `make run-local`.
This command configures Docker to use host networking,
which is helpful for testing against a locally running Anvil, Hardhat, or Hoku node.

### Stop the service

```sh
make stop
```

## Contributing

PRs accepted.

Small note: If editing the README, please conform to
the [standard-readme](https://github.com/RichardLitt/standard-readme) specification.

## License

MIT OR Apache-2.0, © 2024 Hoku Contributors
