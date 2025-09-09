
### Project rules for writing Alkanes contracts

- General style
  - Design contracts as thin responders that delegate to reusable base traits for logic.
  - Keep state in typed `StoragePointer`s, not global statics.
  - Use `MessageDispatch` enums with explicit `#[opcode(n)]` and `#[returns(T)]` to define your contract’s ABI.
  - Prefer composition over inheritance: implement multiple traits (e.g., `AlkaneResponder`, `AuthenticatedResponder`, custom base traits) on a small struct.

### Rule 1: Define your contract interface with MessageDispatch + declare_alkane!
- Use `#[derive(MessageDispatch)]` to map opcodes to handler methods and annotate return shapes with `#[returns(...)]`.
- For each opcode, document parameters and return encoding. (see rule #14 "Encoding").
- Wire your responder with `declare_alkane!` to connect the message enum.
- Example:
```
#[derive(MessageDispatch)]
pub enum AMMPoolMessage {
    #[opcode(0)]
    InitPool { alkane_a: AlkaneId, alkane_b: AlkaneId, factory: AlkaneId },
    #[opcode(1)] AddLiquidity,
    #[opcode(2)] Burn,
    #[opcode(3)] Swap { amount_0_out: u128, amount_1_out: u128, to: AlkaneId, data: Vec<u128> },
    #[opcode(97)] #[returns(u128, u128)] GetReserves,
    #[opcode(99)] #[returns(String)] GetName,
    #[opcode(999)] #[returns(Vec<u8>)] PoolDetails,
}
impl AlkaneResponder for AMMPool {}
declare_alkane! {
    impl AlkaneResponder for AMMPool { type Message = AMMPoolMessage; }
}
```

### Rule 2: Separate orchestration from core logic with “Base” traits
- Put core logic in a trait and implement it on your responder. Keep `lib.rs` thin.
- Separation of concerns - split interface/ABI (message enum), orchestration (trait methods), and storage helpers. 
-Keep business logic in trait impls; keep lib.rs declarative.
- Example:
```
#[derive(Default)]
pub struct AMMFactory();
impl AMMFactoryBase for AMMFactory {}
impl AlkaneResponder for AMMFactory {}
impl AuthenticatedResponder for AMMFactory {}
```
- The `AMMFactoryBase` trait (runtime-side) encapsulates storage, routing, validation, and calls:
```
pub trait AMMFactoryBase: AuthenticatedResponder {
    fn pool_id(&self) -> Result<u128> { /* StoragePointer read */ }
    fn set_pool_id(&self, pool_factory_id: u128) { /* write */ }
    fn beacon_id(&self) -> Result<AlkaneId> { /* read */ }
    fn set_beacon_id(&self, v: AlkaneId) { /* write */ }
    fn create_new_pool(&self, token_a: AlkaneId, token_b: AlkaneId, amount_a: u128, amount_b: u128) -> Result<CallResponse> { /* deploy + init */ }
    /* ... */
}
```

### Rule 3: Use AuthenticatedResponder for access control
- Implement `AuthenticatedResponder` to get `only_owner()` and related auth helpers.
- Restrict dangerous/admin operations (e.g., setting factory IDs, collecting protocol fees).
- Guard all admin/stateful config changes with `only_owner()`.
- Privilege separation: split public flow opcodes vs restricted admin opcodes.
- Example:
```
fn collect_fees(&self, pool_id: AlkaneId) -> Result<CallResponse> {
    self.only_owner()?;
    /* ... */
}
```

### Rule 4: Initialize once with observe_initialization()
- In your initializer, call `self.observe_initialization()?` to prevent re-initialization bugs.
```
fn initialize(&self, total_supply: u128, name: String, symbol: String) -> Result<CallResponse> {
    self.observe_initialization()?;
    /* ... set name/symbol, mint ... */
}
```
```
fn init_factory(&self, pool_factory_id: u128, beacon_id: AlkaneId) -> Result<CallResponse> {
    self.only_owner()?;
    self.observe_initialization()?;
    /* ... */
}
```
- Avoid side effects on replays; design init to be reject-once-initialized with clear error.


### Rule 5: Storage: prefer StoragePointer with explicit keys and typed get/set
- Use keyworded paths for clarity and namespacing.
- For compound keys, build nested selections.
- For typed values, use `get_value::<T>()` and `set_value::<T>()`; 
- For packed bytes, use `into()`/`from` with your types.
- For compound types, wrap with ByteView.
- Namespacing: build hierarchical keys with `.select()` and `.keyword("/")` for composite maps.
- Deterministic ordering: If keys are derived from multiple IDs, canonicalize order (e.g., sort IDs) to avoid duplicates.
- Example:
```
fn factory(&self) -> Result<AlkaneId> {
    let ptr = StoragePointer::from_keyword("/factory_id").get().as_ref().clone();
    let mut cursor = std::io::Cursor::<Vec<u8>>::new(ptr);
    Ok(AlkaneId::new(consume_u128(&mut cursor)?, consume_u128(&mut cursor)?))
}
```
```
fn set_factory(&self, factory_id: AlkaneId) {
    let mut factory_id_pointer = StoragePointer::from_keyword("/factory_id");
    factory_id_pointer.set(Arc::new(factory_id.into()));
}
```
```
fn pool_pointer(&self, a: &AlkaneId, b: &AlkaneId) -> StoragePointer {
    StoragePointer::from_keyword("/pools/").select(&a.clone().into()).keyword("/").select(&b.clone().into())
}
```
- Counters + arrays: maintain …_length and index-addressed storage to enumerate items; encode (count, items...) in responses.


### Rule 6: Define and enforce invariants at the edges
- Validate inputs early; return precise errors via `anyhow!`.
- Explicitly reject unsupported assets in handlers expecting a restricted set.
- Reject invalid inputs ASAP (wrong asset count, duplicates, zero amounts, path length, caller not allowed).
- Assert core invariants post-IO; never rely on caller behavior.
- Stable errors: Use short, stable strings (e.g., "LOCKED", "EXPIRED deadline")—tests and clients assert on these
- Examples: check non-equal tokens, non-zero amounts, path length, deadlines, K-invariant, allowed input token set.
```
if token_a == token_b { return Err(anyhow!("tokens to create the pool cannot be the same")); }
if amount_a == 0 || amount_b == 0 { return Err(anyhow!("input amount cannot be zero")); }
```
```
fn check_inputs(&self, myself: &AlkaneId, parcel: &AlkaneTransferParcel, n: usize) -> Result<()> {
    if parcel.0.len() != n { return Err(anyhow!(format!("{} alkanes sent but expected {} alkane inputs", parcel.0.len(), n))); }
    let (a, b) = self.alkanes_for_self()?;
    if parcel.0.iter().find(|v| myself != &v.id && v.id != a && v.id != b).is_some() {
        return Err(anyhow!("unsupported alkane sent to pool"));
    }
    Ok(())
}
```

### Rule 7: Refund “leftovers” deterministically
- After a sequence of internal and external calls, pay back final balances for all “touched” assets deterministically.
- Pattern:
```
fn _return_leftovers(&self, myself: AlkaneId, result: CallResponse, input_alkanes: AlkaneTransferParcel) -> Result<CallResponse> {
    let mut response = CallResponse::default();
    let mut unique_ids: BTreeSet<AlkaneId> = BTreeSet::new();
    for t in input_alkanes.0 { unique_ids.insert(t.id); }
    for t in result.alkanes.0 { unique_ids.insert(t.id); }
    for id in unique_ids {
        response.alkanes.pay(AlkaneTransfer { id, value: self.balance(&myself, &id) });
    }
    Ok(response)
}
```

### Rule 8: Use Context for caller, self ID, and incoming alkanes
- `context.myself` is the contract’s Alkane ID; `context.caller` is the immediate caller; `context.incoming_alkanes` is the inputs for this call. 
- Validate against these for safety and correctness.

### Rule 9: Prefer forward() to echo inputs when no state change is required
```
fn forward(&self) -> Result<CallResponse> {
    let context = self.context()?;
    Ok(CallResponse::forward(&context.incoming_alkanes))
}
```

### Rule 10: Use reentrancy guards on state-mutating functions
- Implement a simple lock in a shared library and wrap mutating methods in it.
- Critical sections: wrap state-mutating handlers that could be reached via extcall in a lock.
```
pub struct Lock {}
impl Lock {
    pub fn lock<F>(func: F) -> Result<CallResponse>
    where F: FnOnce() -> Result<CallResponse> {
        if Self::lock_pointer().get().len() != 0 && Self::get_lock() == 1 { return Err(anyhow!("LOCKED")); }
        Self::set_lock(1);
        let ret = func();
        Self::set_lock(0);
        ret
    }
}
```
```
fn add_liquidity(&self) -> Result<CallResponse> {
    Lock::lock(|| { /* ...critical section... */ })
}
```
- Lock primitive: keep a simple integer lock in storage; set on entry, clear on exit—even on error paths. 
- Example lock utility:
```
if Lock::get_lock() == 1 { return Err(anyhow!("LOCKED")); } /* set->run->clear */
```

### Rule 11: Math: use U256 (ruint) and wrap for storage
- Use `ruint::Uint<256,4>` for precise AMM math; never rely on u128 for intermediate products.
- Provide a `ByteView` wrapper for storage round-trips and to/from little-endian bytes.
- Use `checked_expr!` and explicit error messages on overflow/underflow conditions.
```
pub type U256 = Uint<256, 4>;
pub trait Sqrt { fn sqrt(self) -> Self; }
impl Sqrt for U256 { /* Newton’s method */ }
```
```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StorableU256(pub U256);
impl ByteView for StorableU256 { /* from_bytes/to_bytes/zero/maximum */ }
```

### Rule 12: Token traits: compose MintableToken for fungible-like behavior
- Implement `MintableToken` for contracts that mint/burn standard tokens.
- Use trait helpers to manage name/symbol/total supply and mint/burn flows.
- Balance reads: use `self.balance(&context.myself, &token_id)` to derive balances; avoid caching unless bounded by a lock.
```
impl MintableToken for AMMPool {}
impl AMMPoolBase for AMMPool {}
```
```
<Self as MintableToken>::set_name_and_symbol_str(self, name, symbol);
response.alkanes.0.push(self.mint(&context, total_supply)?);
```

### Rule 13: ABI stability: sort identifiers consistently
- For ordered pairs, sort `AlkaneId`s to derive stable keys and map lookups.
```
pub fn sort_alkanes((a, b): (AlkaneId, AlkaneId)) -> (AlkaneId, AlkaneId) { if a < b { (a, b) } else { (b, a) } }
```

### Rule 14: External calls and static calls
- Build calls via `Cellpack { target, inputs }` and include `AlkaneTransferParcel` as inputs.
- Use `self.call` for stateful, `self.staticcall` for readonly diagnostics.
- Validate inputs/invariants before any external call to minimize wasted fuel and reduce attack surface.
```
self.call(&Cellpack { target: to.clone(), inputs: extcall_input }, &alkane_transfer.clone(), self.fuel())?;
```
```
let response = self.staticcall(&cellpack, &input_transfer, self.fuel());
```

### Rule 15: Deadlines and time-based logic
- Use block height or header time for deadline checks or TWAP accumulation.
```
fn _check_deadline(&self, height: u64, deadline: u128) -> Result<()> {
    if deadline != 0 && height as u128 > deadline { Err(anyhow!(format!("EXPIRED deadline: block height ({}) > deadline({})", height, deadline))) } else { Ok(()) }
}
```
```
fn _update_cum_prices(&self, reserve0: u128, reserve1: u128) -> Result<()> {
    let block_header = self.block_header()?;
    let current_timestamp = block_header.time;
    /* accumulate price*time in fixed-point */
}
```

### Rule 16: Encoding: encode deterministically and document shapes
- When returning complex structs, provide binary encoders/decoders with explicit sizes and ordering.
- Encode integers as little-endian; define fixed layouts and document sizes.
- For complex responses, add `try_to_vec/from_vec` with strict bounds checks.
```
pub struct PoolInfo { /* fields */ }
impl PoolInfo { pub fn try_to_vec(&self) -> Vec<u8> { /* fixed-width layout */ } pub fn from_vec(bytes: &[u8]) -> Result<Self> { /* parse */ } }
```
- Expose via opcode with `#[returns(Vec<u8>)]`.


### Rule 17: Deployment and upgradeability via beacons/proxies
- Store factory implementation IDs and a `beacon_id`.
- Deploy proxies, then init target logic through the proxy’s address.
- Keep upgrade slots distinct and documented.
```
/* deploys proxy */ self.call(&Cellpack { target: AlkaneId { block: 6, tx: self.pool_id()? }, inputs: vec![0x7fff, beacon_id.block, beacon_id.tx] }, &AlkaneTransferParcel::default(), self.fuel())?;
/* inits proxy */ self.call(&Cellpack { target: AlkaneId { block: 2, tx: pool_id.tx }, inputs: vec![0, a.block, a.tx, b.block, b.tx, context.myself.block, context.myself.tx] }, &input_transfer, self.fuel())?;
```
- Build and cache standard beacons and proxies with the standard crates from alkanes-rs (`alkanes-std-auth-token`, `alkanes-std-owned-token`, `alkanes-std-beacon-proxy`, `alkanes-std-upgradeable-beacon`, `alkanes-std-upgradeable`).


### Rule 18: Test structure: simulate blocks/transactions, assert traces and balances
- Organize under `src/tests`, split helpers from cases.
- Use `alkanes::tests::helpers` to construct cellpacks and index blocks, and to assert revert messages.
- Avoid randomness; keep fixed ordering and encodings in assertions.
- Example test pattern:
```
clear();
let (init_block, mut runtime_balances, deployment_ids) = test_amm_pool_init_fixture(...)?;
let mut swap_block = create_block_with_coinbase_tx(block_height);
insert_swap_exact_tokens_for_tokens(..., &mut swap_block, input_outpoint, &deployment_ids);
index_block(&swap_block, block_height)?;
check_swap_lp_balance(...)?; check_swap_runtime_balance(...)?;
```
- Check revert contexts:
```
assert_revert_context(&outpoint, "EXPIRED deadline")?;
```
- Inspect return data via `view::trace`:
```
let trace_data: Trace = view::trace(&outpoint)?.try_into()?;
if let TraceEvent::ReturnContext(resp) = last { let data = &resp.inner.data; /* assert */ }
```

### Rule 19: Build automation and test artifact embedding
- Provide a `build.rs` that compiles each "alkanes/*" crate to wasm, gzips the binary, and generates Rust modules with embedded bytes for tests to consume.
```
std::env::set_current_dir(&crates_dir)?;
build_alkane(wasm_str, vec![])?;
let f: Vec<u8> = fs::read(Path::new(&wasm_str).join("wasm32-unknown-unknown").join("release").join(subbed.clone() + ".wasm"))?;
let compressed = compress(f.clone())?;
fs::write(...join(subbed.clone() + ".wasm.gz"), &compressed)?;
fs::write(&write_dir.join("std").join(subbed.clone() + "_build.rs"), format!("use hex_lit::hex; ... get_bytes() -> Vec<u8> {{ (&hex!(\"{}\")).to_vec() }}", hex::encode(&f)))?;
```
- Add the generated "std/*_build.rs" modules to "src/tests/std/mod.rs".

### Rule 20: Features, networks, and debug logging
- Use Cargo features to toggle network or debug behavior.
- Example feature gating:
```
[features]
test = []
testnet = []
dogecoin = []
...
debug-log = ["alkanes/debug-log"]
```

### Rule 21: Use standard crates and exports from alkanes-rs
- Core crates:
  - `alkanes-runtime`: `declare_alkane!`, `MessageDispatch`, `runtime::AlkaneResponder`, `auth::AuthenticatedResponder`, `storage::StoragePointer`, logging helpers.
  - `alkanes-support`: `cellpack::Cellpack`, `context::Context`, `id::AlkaneId`, `parcel::{AlkaneTransfer, AlkaneTransferParcel}`, `response::CallResponse`, `checked_expr!`, `utils::{shift, shift_or_err}`.
  - `alkanes-std-factory-support`: `MintableToken` and token metadata helpers.
  - `protorune-support` and `ordinals` for test scaffolding/integration with runes/Protostones.
  - `metashrew-support`/`metashrew-core` for index pointers, byte view, test utils.
- In tests, enable `alkanes` with `test-utils` feature.


### Rule 22: Contract-to-contract metadata lookups
- During cross-contract UX, fallback gracefully if calls don’t return data.
```
let name_a = match self.call(&Cellpack{target: alkane_a, inputs: vec![99]}, &AlkaneTransferParcel(vec![]), self.fuel()) {
    Ok(resp) => if resp.data.is_empty() { format!("{},{}", alkane_a.block, alkane_a.tx) } else { String::from_utf8_lossy(&resp.data).to_string() },
    Err(_) => format!("{},{}", alkane_a.block, alkane_a.tx),
};
```

### Rule 23: Fuel and costs
- Always pass `self.fuel()` for `call/staticcall` budgeting, and be mindful of the number of external calls within a single opcode.


### Rule 24: Document ABI and state schema
- For each contract, write a short doc enumerating:
  - Opcodes, parameters, and return encodings.
  - Storage keys and their types/encodings.
  - External call targets and assumptions.
  - Error messages and invariants.
- Keep this doc next to `lib.rs` or in `docs/`.

### Rule 25: CI recommendations
- Build `wasm32-unknown-unknown --release` for all "alkanes/*"  crates.
- Run wasm-bindgen tests (`wasm-bindgen-test`) for in-browser or headless environments.
- Validate that `build.rs` regenerates test std modules and that tests compile without network-dependent side effects.

### Rule 26: Security checklist before shipping
- Access control on admin ops (`AuthenticatedResponder`).
- Reentrancy lock wrapping every state-mutating path that can be called during an extcall.
- Invariant checks (K, deadlines, path length, token set correctness).
- Overflow-safe math with U256; convert to u128 only at the perimeter.
- All money movements audited via “leftover refunds” pattern.


### Rule 27: Witness-driven data attachments (set_data pattern)
- Read the full transaction from context and extract a witness payload to seed on-chain data (e.g., images/metadata) at initialization.
- Convention: read from input index 0 during initialization; store raw bytes under `/data`, and decompress on reads for UX.
```
fn set_data(&self) -> Result<()> {
    let tx = consensus_decode::<Transaction>(&mut std::io::Cursor::new(CONTEXT.transaction()))?;
    let data: Vec<u8> = find_witness_payload(&tx, 0).unwrap_or_else(|| vec![]);
    self.data_pointer().set(Arc::new(data));
    Ok(())
}
fn data(&self) -> Vec<u8> {
    gz::decompress(self.data_pointer().get().as_ref().clone()).unwrap_or_else(|_| vec![])
}
```
- Call `set_data()` inside your initializer after `observe_initialization()` to persist the deployment’s witness payload.


### Rule 28: Per-transaction gating via txid sets
- Derive the caller transaction id and gate sensitive actions (e.g., free mints) to one-per-tx using a persistent set keyed by txid bytes.
```
trait ContextExt { fn transaction_id(&self) -> Result<Txid>; }
impl ContextExt for Context {
    fn transaction_id(&self) -> Result<Txid> {
        Ok(consensus_decode::<Transaction>(&mut std::io::Cursor::new(CONTEXT.transaction()))?.compute_txid())
    }
}
fn has_tx_hash(&self, txid: &Txid) -> bool {
    StoragePointer::from_keyword("/tx-hashes/")
        .select(&txid.as_byte_array().to_vec())
        .get_value::<u8>() == 1
}
fn add_tx_hash(&self, txid: &Txid) { StoragePointer::from_keyword("/tx-hashes/")
    .select(&txid.as_byte_array().to_vec()).set_value::<u8>(0x01); }
```
- Use early returns when a txid is already present; record the txid immediately before proceeding with mint/state changes.


### Rule 29: Sentinel configuration values
- Use sentinel encodings for “unbounded” configuration. Example: a cap of 0 means unlimited; store as `u128::MAX` in `/cap`.
```
fn set_cap(&self, v: u128) { self.cap_pointer().set_value::<u128>(if v == 0 { u128::MAX } else { v }); }
```
- Clearly document sentinel semantics in your ABI docs.


### Rule 30: String packing into integers for ABI simplicity
- Pack short ASCII strings into little-endian integers to keep opcodes strictly numeric. For names, accept two `u128` parts and concatenate; symbols can fit into one `u128`.
```
pub fn trim(v: u128) -> String {
    String::from_utf8(v.to_le_bytes().into_iter().filter(|b| *b != 0).collect()).unwrap()
}
#[derive(Default, Clone, Copy)]
pub struct TokenName { pub part1: u128, pub part2: u128 }
impl From<TokenName> for String { fn from(n: TokenName) -> Self { format!("{}{}", trim(n.part1), trim(n.part2)) } }
```
- Document the endianess and maximum sizes; prefer explicit sizes in encoding docs (see Rule 16).


### Rule 31: Standard storage keys for mintable/fungible-like contracts
- Suggested keys for consistent schema and tooling:
  - `/name` (bytes, UTF-8)
  - `/symbol` (bytes, UTF-8)
  - `/totalsupply` (u128)
  - `/minted` (u128, count of successful mint operations)
  - `/value-per-mint` (u128)
  - `/cap` (u128, 0 interpreted as unlimited per Rule 29)
  - `/data` (bytes; witness payload, often gz-compressed)
  - `/initialized` (u8 flag)
  - `/tx-hashes/` (map: txid bytes -> u8 flag)


### Rule 32: Recommended token opcode map
- For token-like contracts, prefer a stable set of opcodes to ease wallet/indexer integrations:
  - `0`: Initialize(token_units, value_per_mint, cap, name_part1, name_part2, symbol)
  - `77`: MintTokens()
  - `88`: SetNameAndSymbol(name_part1, name_part2, symbol)
  - `99`: GetName() -> String
  - `100`: GetSymbol() -> String
  - `101`: GetTotalSupply() -> u128
  - `102`: GetCap() -> u128
  - `103`: GetMinted() -> u128
  - `104`: GetValuePerMint() -> u128
  - `1000`: GetData() -> Vec<u8> (reserve higher ranges for bulk/aux data)
- Keep opcodes and return shapes documented next to the contract (see Rule 24).


### Rule 33: Build script hygiene for test artifacts
- When embedding wasm into tests, avoid recursive `build.rs` invocation and isolate the wasm target:
```
// guard recursion
if std::env::var("FREE_MINT_BUILD_IN_PROGRESS").is_ok() { return; }
std::env::set_var("FREE_MINT_BUILD_IN_PROGRESS", "1");
// isolate target and emit hex-embedded test module
Command::new("cargo").env("CARGO_TARGET_DIR", wasm_out).arg("build").arg("--release").spawn()?.wait()?;
fs::write(write_dir.join("std").join(mod_name + "_build.rs"), format!("use hex_lit::hex; ... get_bytes() -> Vec<u8> {{ (&hex!(\"{}\")).to_vec() }}", hex::encode(&wasm_bytes)))?;
```
- Optionally gzip the `.wasm` alongside for distribution and debugging.


### Rule 34: Test hygiene for wasm unit tests and integration tests
- For wasm-bindgen unit tests, clear contract storage between tests to avoid inter-test coupling:
```
fn reset_test_storage() {
    for k in ["/initialized","/name","/symbol","/totalsupply","/minted","/value-per-mint","/cap","/data","/tx-hashes"] {
        StoragePointer::from_keyword(k).set(Arc::new(Vec::new()));
    }
}
```
- For integration tests, construct init/mint blocks using helpers, then index and assert balances:
```
init_with_multiple_cellpacks_with_tx(vec![free_mint_build::get_bytes()], vec![Cellpack{ target, inputs: vec![0, token_units, value_per_mint, cap, name_part1, name_part2, symbol] }]);
index_block(&block, height)?; // assert balances via sheets or traces
```

### Libraries/Crates quick-reference
- alkanes-runtime: `AlkaneResponder`, `AuthenticatedResponder`, `declare_alkane!`, `MessageDispatch`, `StoragePointer`, `Context`, `call/staticcall`, `fuel`, `balance`, `block_header`, `height`.
- alkanes-support: `AlkaneId`, `Cellpack`, `AlkaneTransfer(Parcel)`, `CallResponse`, `checked_expr!`, parsing helpers.
- alkanes-std-factory-support: `MintableToken` (name/symbol/total supply, mint/burn).
- metashrew-(core/support): `IndexPointer`, `KeyValuePointer`, `ByteView`, test utils.
- protorune(-support): protostone edicts, test helpers.
- ordinals: runestone/etching constructs (tests).
- ruint: `Uint<256,4>` for fixed-width 256-bit math.

