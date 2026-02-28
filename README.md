# token-metadata-cli

A simple Rust CLI to create or update Metaplex Token Metadata on Solana.

## Build

```bash
cargo build --release
```

## Usage

### Create metadata for an existing mint

```bash
# Minimal (uses default keypair + devnet)
token-metadata-cli create \
  --mint <MINT_ADDRESS> \
  --name "My Token" \
  --symbol "MTK"

# With URI and custom keypair/RPC
token-metadata-cli -k /path/to/keypair.json -u https://api.devnet.solana.com create \
  --mint <MINT_ADDRESS> \
  --name "My Token" \
  --symbol "MTK" \
  --uri "https://arweave.net/your-metadata.json" \
  --seller-fee-basis-points 0 \
  --mutable true
```

### Update existing metadata

```bash
# Update name only
token-metadata-cli update \
  --mint <MINT_ADDRESS> \
  --name "New Name"

# Update multiple fields
token-metadata-cli update \
  --mint <MINT_ADDRESS> \
  --name "New Name" \
  --symbol "NEW" \
  --uri "https://arweave.net/new-metadata.json"
```

### Global options

| Flag | Description | Default |
|------|-------------|---------|
| `-k, --keypair` | Path to keypair file | `~/.config/solana/id.json` |
| `-u, --url` | Solana RPC URL | `https://api.devnet.solana.com` |

## Notes

- You must be the **mint authority** to create metadata
- You must be the **update authority** to update metadata
- The keypair file is the standard Solana CLI format (JSON array of bytes)
- For mainnet, change the URL: `-u https://api.mainnet-beta.solana.com`
