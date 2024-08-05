## Faucet

tHOKU ERC20 contract can be found [here](https://github.com/amazingdatamachine/contracts)

### Deployment

#### Build faucet docker image
`make build`

#### Run the container

- Private key is `tHOKU`'s deployer key
-  Token address is the `tHOKU`'s (proxy) address

`PRIVATE_KEY=<> TOKEN_ADDRESS=<> make run`

#### Stop
`make stop`
