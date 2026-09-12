//! `FreshCredit` Security CLI Tool
//!
//! Provides key management and encryption utilities for `FreshCredit`.
//!
//! # Usage
// TAG: surface=security owner=security-team rule=SEC-001
//! ```bash
//! # Generate a new encryption key
//! fc-security generate-key
//!
//! # Test encryption with a key
//! fc-security test-encrypt --key <hex-key> --plaintext "my-secret"
//!
//! # Validate configuration
//! fc-security validate-config
//! ```

use clap::{Parser, Subcommand};
use freshcredit_security::{
    decrypt_token, encrypt_token, generate_base64_key, get_encryption_config, TokenEncryptor,
    KEY_SIZE,
};

#[derive(Parser)]
#[command(name = "fc-security")]
#[command(about = "FreshCredit Security CLI - Key management and encryption utilities")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new AES-256 encryption key
    GenerateKey {
        /// Output format: hex (default) or base64
        #[arg(short, long, default_value = "hex")]
        format: String,
    },

    /// Test encryption/decryption with a key
    TestEncrypt {
        /// Hex-encoded encryption key (64 characters)
        #[arg(short, long)]
        key: String,

        // TAG: surface=security owner=security-team rule=SEC-001
        /// Plaintext to encrypt
        #[arg(short, long)]
        plaintext: String,
    },

    /// Validate current encryption configuration
    ValidateConfig,

    /// Encrypt a token using the global configuration
    Encrypt {
        /// Plaintext token to encrypt
        #[arg(short, long)]
        plaintext: String,
    },

    /// Decrypt a token using the global configuration
    Decrypt {
        /// Encrypted token to decrypt
        #[arg(short, long)]
        ciphertext: String,
    },
}

fn handle_generate_key(format: &str) {
    if format == "base64" {
        let b64 = generate_base64_key();
        println!("Generated AES-256 key (base64):");
        println!("{b64}");
    } else {
        let key = freshcredit_security::encryption::generate_key();
        let hex = hex::encode(key);
        println!("Generated AES-256 key (hex):");
        println!("{hex}");
    }
    println!("\nSet this as FRESHCREDIT_TOKEN_ENCRYPTION_KEY environment variable.");
}

// TAG: surface=security owner=security-team rule=SEC-001
fn handle_test_encrypt(key: &str, plaintext: &str) {
    let expected_len = KEY_SIZE * 2;
    let actual_len = key.len();
    if actual_len != expected_len {
        eprintln!("Error: Key must be {expected_len} hex characters (got {actual_len})");
        std::process::exit(1);
    }

    let encryptor = TokenEncryptor::from_hex_key(key).unwrap_or_else(|e| {
        eprintln!("Invalid key: {e}");
        std::process::exit(1);
    });

    let ciphertext = encryptor.encrypt(plaintext).unwrap_or_else(|e| {
        eprintln!("Encryption failed: {e}");
        std::process::exit(1);
    });

    println!("Plaintext:  {plaintext}");
    println!("Ciphertext: {ciphertext}");

    let decrypted = encryptor.decrypt(&ciphertext).unwrap_or_else(|e| {
        eprintln!("Decryption failed: {e}");
        std::process::exit(1);
    });

    println!("Decrypted:  {decrypted}");
    if decrypted == plaintext {
        println!("\n✅ Encryption/decryption roundtrip successful!");
    } else {
        eprintln!("\n❌ Decrypted text doesn't match original!");
        std::process::exit(1);
    }
}

// TAG: surface=security owner=security-team rule=SEC-001
fn handle_validate_config() {
    println!("Validating encryption configuration...\n");

    let config = get_encryption_config();

    println!("FRESHCREDIT_ENCRYPTION_ENABLED: {}", config.enabled);
    println!("Encryption available: {}", config.is_available());

    if config.is_available() {
        println!("\n✅ Encryption is properly configured!");

        let test = "test-token-12345";
        let encrypted = encrypt_token(test).unwrap_or_else(|e| {
            eprintln!("❌ Encryption failed: {e}");
            std::process::exit(1);
        });
        let decrypted = decrypt_token(&encrypted).unwrap_or_else(|e| {
            eprintln!("❌ Encryption roundtrip test failed: {e}");
            std::process::exit(1);
        });

        if decrypted == test {
            println!("✅ Encryption roundtrip test passed!");
        } else {
            eprintln!("❌ Encryption roundtrip test failed - mismatch!");
            std::process::exit(1);
        }
    } else if config.enabled {
        println!("\n⚠️ Encryption is enabled but no key is configured.");
        println!("Set FRESHCREDIT_TOKEN_ENCRYPTION_KEY with a 64-character hex key.");
    } else {
        println!("\n⚠️ Encryption is disabled.");
        println!("Set FRESHCREDIT_ENCRYPTION_ENABLED=true to enable.");
    }
}

// TAG: surface=security owner=security-team rule=SEC-001
fn handle_encrypt(plaintext: &str) {
    match encrypt_token(plaintext) {
        Ok(encrypted) => println!("{encrypted}"),
        Err(e) => {
            eprintln!("❌ Encryption failed: {e}");
            std::process::exit(1);
        }
    }
}

fn handle_decrypt(ciphertext: &str) {
    match decrypt_token(ciphertext) {
        Ok(decrypted) => println!("{decrypted}"),
        Err(e) => {
            eprintln!("❌ Decryption failed: {e}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateKey { format } => handle_generate_key(&format),
        Commands::TestEncrypt { key, plaintext } => handle_test_encrypt(&key, &plaintext),
        Commands::ValidateConfig => handle_validate_config(),
        Commands::Encrypt { plaintext } => handle_encrypt(&plaintext),
        Commands::Decrypt { ciphertext } => handle_decrypt(&ciphertext),
    }
}
