# Vaulth

Just a flexible authentication server

## Account providers

### External

- Google
- Microsoft
- Facebook
- Twitter
- GitHub
- Discord
- Steam

### Local

Local accounts use Argon2i 0x13 to securely store passwords. The hashing settings can be changed in the config file.

## Running

```
vaulth [CONFIG]
```

If no config file is specified, it defaults to `vaulth.json`.

## Configuration

See [example](vaulth.example.json5) (the comments are present for clarity only, parsing will fail if the config file uses JSON5).

### Generating JWT keypair

The JWT signature algorithm used by Vaulth is ES384.

```
openssl ecparam -genkey -name secp384r1 -noout -out private.pem
openssl ec -in private.pem -pubout -out public.pem
```

## Building

### PostgreSQL

```
cargo build --release --features postgres
```

### MySQL

```
cargo build --release --features mysql
```
