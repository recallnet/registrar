## Faucet

tHOKU ERC20 contract can be found [here](https://github.com/amazingdatamachine/contracts)

### Deployment

#### Build faucet docker image
```sh
make build
```

#### Run the service

- Private key is `tHOKU`'s deployer key
-  Token address is the `tHOKU`'s (proxy) address

```sh
PRIVATE_KEY=<> TOKEN_ADDRESS=<> make run
```

#### Stop the service
```sh 
make stop
```

#### Usage
To get 5e18 tokens on a given address:


```sh
curl -X POST -H 'Content-Type: application/json' 'http://<faucet host>/send' --data-raw '{"address":"0xfoobar"}'
```



