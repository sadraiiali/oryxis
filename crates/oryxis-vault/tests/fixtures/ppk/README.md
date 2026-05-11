# PPK test fixtures

These `.ppk` and `.pem` files come from
[philr/putty-key](https://github.com/philr/putty-key/tree/master/test/fixtures)
(MIT-licensed, see `LICENSE-MIT`). They were generated with the real
`puttygen` binary and are used here to validate byte-exact
compatibility of our PPK parser with PuTTY's output.

All encrypted fixtures use the passphrase `Test Passphrase`. The
`.pem` files are OpenSSH/OpenSSL exports of the same private key
material, used to cross-check fingerprints.
