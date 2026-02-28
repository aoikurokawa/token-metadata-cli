use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mpl_token_metadata::{
    ID as TOKEN_METADATA_PROGRAM_ID,
    instructions::{CreateMetadataAccountV3Builder, UpdateMetadataAccountV2Builder},
    types::DataV2,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer, read_keypair_file},
    transaction::Transaction,
};
use std::str::FromStr;

#[derive(Parser)]
#[command(name = "token-metadata-cli")]
#[command(
    about = "Create or update token metadata on Solana using Metaplex Token Metadata program"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the payer/authority keypair file
    #[arg(short, long, default_value = "~/.config/solana/id.json")]
    keypair: String,

    /// Solana RPC URL
    #[arg(short, long, default_value = "https://api.devnet.solana.com")]
    url: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Create metadata for an existing token mint
    Create {
        /// Token mint address
        #[arg(short, long)]
        mint: String,

        /// Token name
        #[arg(short, long)]
        name: String,

        /// Token symbol
        #[arg(short, long)]
        symbol: String,

        /// Metadata URI (JSON file URL)
        #[arg(long, default_value = "")]
        uri: String,

        /// Whether metadata should be mutable
        #[arg(long, default_value_t = true)]
        mutable: bool,

        /// Seller fee basis points (0-10000)
        #[arg(long, default_value_t = 0)]
        seller_fee_basis_points: u16,
    },
    /// Update metadata for an existing token mint
    Update {
        /// Token mint address
        #[arg(short, long)]
        mint: String,

        /// New token name (optional)
        #[arg(short, long)]
        name: Option<String>,

        /// New token symbol (optional)
        #[arg(short, long)]
        symbol: Option<String>,

        /// New metadata URI (optional)
        #[arg(long)]
        uri: Option<String>,
    },
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

fn load_keypair(path: &str) -> Result<Keypair> {
    let expanded = expand_tilde(path);
    read_keypair_file(&expanded)
        .map_err(|e| anyhow::anyhow!("Failed to read keypair from '{}': {}", expanded, e))
}

/// Derive the metadata PDA for a given mint
fn find_metadata_pda(mint: &Pubkey) -> Pubkey {
    let seeds = &[
        b"metadata".as_ref(),
        TOKEN_METADATA_PROGRAM_ID.as_ref(),
        mint.as_ref(),
    ];
    Pubkey::find_program_address(seeds, &TOKEN_METADATA_PROGRAM_ID).0
}

fn create_metadata(
    client: &RpcClient,
    payer: &Keypair,
    mint: &Pubkey,
    name: String,
    symbol: String,
    uri: String,
    seller_fee_basis_points: u16,
    is_mutable: bool,
) -> Result<()> {
    let metadata_pda = find_metadata_pda(mint);

    println!("Creating metadata...");
    println!("  Mint:         {}", mint);
    println!("  Metadata PDA: {}", metadata_pda);
    println!("  Name:         {}", name);
    println!("  Symbol:       {}", symbol);
    println!(
        "  URI:          {}",
        if uri.is_empty() { "(empty)" } else { &uri }
    );
    println!("  Mutable:      {}", is_mutable);

    let data = DataV2 {
        name,
        symbol,
        uri,
        seller_fee_basis_points,
        creators: None,
        collection: None,
        uses: None,
    };

    let ix = CreateMetadataAccountV3Builder::new()
        .metadata(metadata_pda)
        .mint(*mint)
        .mint_authority(payer.pubkey())
        .payer(payer.pubkey())
        .update_authority(payer.pubkey(), true)
        .data(data)
        .is_mutable(is_mutable)
        .instruction();

    let recent_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    let signature = client
        .send_and_confirm_transaction_with_spinner(&tx)
        .context("Failed to send create metadata transaction")?;

    println!("\nMetadata created successfully!");
    println!("  Signature: {}", signature);
    println!(
        "  Explorer:  https://explorer.solana.com/tx/{}?cluster=devnet",
        signature
    );

    Ok(())
}

fn update_metadata(
    client: &RpcClient,
    payer: &Keypair,
    mint: &Pubkey,
    name: Option<String>,
    symbol: Option<String>,
    uri: Option<String>,
) -> Result<()> {
    let metadata_pda = find_metadata_pda(mint);

    // Fetch existing metadata account to get current values
    let metadata_account = client
        .get_account_data(&metadata_pda)
        .context("Failed to fetch metadata account. Does it exist?")?;

    // Parse existing metadata using borsh
    // The metadata account has an offset; skip the first byte (key discriminator)
    // and parse the rest. For simplicity we'll use mpl_token_metadata's deserialization.
    use borsh::BorshDeserialize;
    use mpl_token_metadata::accounts::Metadata;

    let existing = Metadata::from_bytes(&metadata_account)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize metadata: {}", e))?;

    let updated_name = name.unwrap_or(existing.name.clone());
    let updated_symbol = symbol.unwrap_or(existing.symbol.clone());
    let updated_uri = uri.unwrap_or(existing.uri.clone());

    println!("Updating metadata...");
    println!("  Mint:         {}", mint);
    println!("  Metadata PDA: {}", metadata_pda);
    println!(
        "  Name:         {} -> {}",
        existing.name.trim_end_matches('\0'),
        updated_name
    );
    println!(
        "  Symbol:       {} -> {}",
        existing.symbol.trim_end_matches('\0'),
        updated_symbol
    );
    println!(
        "  URI:          {} -> {}",
        existing.uri.trim_end_matches('\0'),
        updated_uri
    );

    let new_data = DataV2 {
        name: updated_name,
        symbol: updated_symbol,
        uri: updated_uri,
        seller_fee_basis_points: existing.seller_fee_basis_points,
        creators: existing.creators,
        collection: existing
            .collection
            .map(|c| mpl_token_metadata::types::Collection {
                verified: c.verified,
                key: c.key,
            }),
        uses: existing.uses.map(|u| mpl_token_metadata::types::Uses {
            use_method: u.use_method,
            remaining: u.remaining,
            total: u.total,
        }),
    };

    let ix = UpdateMetadataAccountV2Builder::new()
        .metadata(metadata_pda)
        .update_authority(payer.pubkey())
        .data(new_data)
        .instruction();

    let recent_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    let signature = client
        .send_and_confirm_transaction_with_spinner(&tx)
        .context("Failed to send update metadata transaction")?;

    println!("\nMetadata updated successfully!");
    println!("  Signature: {}", signature);
    println!(
        "  Explorer:  https://explorer.solana.com/tx/{}?cluster=devnet",
        signature
    );

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let payer = load_keypair(&cli.keypair)?;
    let client = RpcClient::new_with_commitment(&cli.url, CommitmentConfig::confirmed());

    println!("Using RPC:    {}", cli.url);
    println!("Using wallet: {}\n", payer.pubkey());

    match cli.command {
        Commands::Create {
            mint,
            name,
            symbol,
            uri,
            mutable,
            seller_fee_basis_points,
        } => {
            let mint_pubkey = Pubkey::from_str(&mint).context("Invalid mint address")?;
            create_metadata(
                &client,
                &payer,
                &mint_pubkey,
                name,
                symbol,
                uri,
                seller_fee_basis_points,
                mutable,
            )?;
        }
        Commands::Update {
            mint,
            name,
            symbol,
            uri,
        } => {
            let mint_pubkey = Pubkey::from_str(&mint).context("Invalid mint address")?;
            update_metadata(&client, &payer, &mint_pubkey, name, symbol, uri)?;
        }
    }

    Ok(())
}
