# CONTEXT: `blockchain` Agent Workspace

## YOU ARE HERE
**Path:** `freshcredit/apps/blockchain/`  
**Type:** Blockchain Explorer Service / Block Verification UI  
**Parent Isolation:** `freshcredit/apps/` — sibling apps are separate services, not imports

## YOUR SCOPE
Blockchain explorer web application for FreshCredit's Substrate-based blockchain. Provides block and transaction visualization, verification, and WebSocket-based live updates.

**What you own:**
- Web-based block explorer interface
- Transaction verification via blockchain proofs
- Block detail views and transaction inspection
- WebSocket client for live blockchain updates
- Block/transaction search and filtering
- Verification status display

## YOUR BOUNDARIES

### Inbound (what can call you)
- **Browser Users:** Navigating to blockchain explorer UI
- **`apps/web/`** — May link to explorer from main app
- **`apps/api/`** — May reference explorer URLs

### Outbound (what you can call)
- **`blockchain-client` crate** — Core blockchain client for Substrate interaction
- **Substrate Node RPC:** WebSocket connection for blockchain data
- **Workspace Crates:** `freshcredit-types`, `freshcredit-config`, `freshcredit-security`

### Forbidden (what you must NOT do)
- ❌ NEVER import from `apps/web/` — Web is a separate application
- ❌ NEVER import from `apps/engine/` — Use the `blockchain-client` crate instead
- ❌ NEVER import from `apps/simulator/` — SIM-001 violation
- ❌ NEVER put business logic here — Only display and verification

## LOCAL STRUCTURE
```
apps/blockchain/
├── src/
│   ├── main.rs              # Explorer server entry point
│   ├── lib.rs               # Library exports
│   ├── error.rs             # Error handling
│   ├── handlers.rs          # HTTP route handlers
│   ├── explorer_service.rs  # Block explorer service logic
│   ├── block_detail_view.rs # Block detail page
│   ├── block_explorer.rs    # Explorer state and logic
│   ├── consensus.rs         # Consensus-related functions
│   ├── models.rs            # Data models
│   ├── config.rs            # Configuration
│   ├── ws_handler.rs        # WebSocket handler
│   ├── block.rs             # Block data structures
│   ├── verification.rs      # Proof verification
│   ├── templates/           # Askama HTML templates
│   └── templates.rs         # Template rendering
├── templates/               # Askama HTML templates
│   └── *.html
├── static/                  # Static assets (CSS, JS)
├── tests/                   # Integration tests
└── Cargo.toml
```

## EXTERNAL CONTRACTS
- **Substrate RPC:** `wss://` WebSocket connection to blockchain node
- **Blockchain Client:** `blockchain-client` crate provides high-level API
- **Block Structure:** Substrate standard block format
- **Verification:** Blake2b hashing, Merkle proofs

## ISOLATION RULES
1. **Use `blockchain-client` crate only** — All blockchain interaction through this crate
2. **Never import from sibling apps** — Explorer is a standalone service
3. **Read-only operations primarily** — Explorer mainly queries blockchain state
4. **WebSocket for live updates** — Use Substrate subscriptions for real-time data
5. **SIM-001 compliance** — No dependencies on `sim-core` or simulator crates

## DEPENDENCY SUMMARY
```toml
[dependencies]
# Core workspace
freshcredit-types = { path = "../../crates/core/types" }
freshcredit-config = { path = "../../crates/core/config" }
freshcredit-security = { path = "../../crates/core/security" }

# Blockchain
blockchain-client = { path = "../../crates/blockchain-client" }

# Web framework
axum = "0.7"
tokio = { version = "1", features = ["full"] }

# Templates
askama = "0.12"

# Substrate
subxt = "0.37"
```

## KEY ENDPOINTS
```
GET /                    # Explorer home
GET /block/:block_hash   # Block detail
GET /tx/:tx_hash         # Transaction detail
GET /search              # Block/tx search
WS /ws                   # Live update WebSocket
```

## CONFIGURATION
```bash
BLOCKCHAIN_RPC_URL=wss://substrate-node.example.com
BLOCKCHAIN_EXPLORER_PORT=8080
```
