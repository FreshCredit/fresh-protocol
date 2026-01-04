//! FreshCredit Security CLI Tool
//!
//! Provides key management and encryption utilities for FreshCredit.
//!
//! # Usage
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
    encrypt_token, decrypt_token, generate_base64_key, get_encryption_config,
    TokenEncryptor, KEY_SIZE,
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateKey { format } => {
            match format.as_str() {
                "base64" => {
                    let b64 = generate_base64_key();
                    println!("Generated AES-256 key (base64):");
                    println!("{b64}");
                }
                _ => {
                    let key = freshcredit_security::encryption::generate_key();
                    let hex = hex::encode(key);
                    println!("Generated AES-256 key (hex):");
                    println!("{hex}");
                }
            }
            println!("\nSet this as FRESHCREDIT_TOKEN_ENCRYPTION_KEY environment variable.");
        }

        Commands::TestEncrypt { key, plaintext } => {
            let expected_len = KEY_SIZE * 2;
            let actual_len = key.len();
            if actual_len != expected_len {
                eprintln!("Error: Key must be {expected_len} hex characters (got {actual_len})");
                std::process::exit(1);
            }

            match TokenEncryptor::from_hex_key(&key) {
                Ok(encryptor) => {
                    match encryptor.encrypt(&plaintext) {
                        Ok(ciphertext) => {
                            println!("Plaintext:  {plaintext}");
                            println!("Ciphertext: {ciphertext}");

                            // Verify decryption
                            match encryptor.decrypt(&ciphertext) {
                                Ok(decrypted) => {
                                    println!("Decrypted:  {decrypted}");
                                    if decrypted == plaintext {
                                        println!("\n✅ Encryption/decryption roundtrip successful!");
                                    } else {
                                        eprintln!("\n❌ Decrypted text doesn't match original!");
                                        std::process::exit(1);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Decryption failed: {e}");
                                    std::process::exit(1);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Encryption failed: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Invalid key: {e}");
                    std::process::exit(1);
                }
            }
        }

        Commands::ValidateConfig => {
            println!("Validating encryption configuration...\n");

            let config = get_encryption_config();

            println!("FRESHCREDIT_ENCRYPTION_ENABLED: {}", config.enabled);
            println!("Encryption available: {}", config.is_available());

            if config.is_available() {
                println!("\n✅ Encryption is properly configured!");
                
                // Test roundtrip
                let test = "test-token-12345";
                let encrypted = encrypt_token(test);
                let decrypted = decrypt_token(&encrypted);
                
                if decrypted == test {
                    println!("✅ Encryption roundtrip test passed!");
                } else {
                    eprintln!("❌ Encryption roundtrip test failed!");
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

        Commands::Encrypt { plaintext } => {
            let encrypted = encrypt_token(&plaintext);
            println!("{encrypted}");
        }

        Commands::Decrypt { ciphertext } => {
            let decrypted = decrypt_token(&ciphertext);
            println!("{decrypted}");
        }
    }
}

