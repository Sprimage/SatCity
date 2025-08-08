use crate::helpers::{decode_nfts, decode_players, encode_nfts, encode_players, encode_txs};
use crate::mempool::Transaction;
use crate::state::State;
use bincode::enc::write::Writer;
use cairo1_run::error::Error;
use cairo1_run::{cairo_run_program, Cairo1RunConfig, FuncArg};
use cairo_air::utils::ProofFormat;
use cairo_air::PreProcessedTraceVariant;
use cairo_lang_sierra::program::Program as SierraProgram;
use cairo_vm::stdlib::collections::HashMap;
use cairo_vm::types::layout_name::LayoutName;
use cairo_vm::vm::errors::trace_errors::TraceError;
use cairo_vm::Felt252;
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use stwo_cairo_adapter::builtins::MemorySegmentAddresses;
use stwo_cairo_adapter::memory::{MemoryBuilder, MemoryConfig, MemoryEntry as StwoMemoryEntry};
use stwo_cairo_adapter::vm_import::{adapt_to_stwo_input, RelocatedTraceEntry as StwoRelocatedTraceEntry};
use stwo_cairo_adapter::{ProverInput, PublicSegmentContext};
use stwo_cairo_prover::prover::{default_prod_prover_parameters, prove_cairo};
use stwo_cairo_prover::stwo_prover::core::backend::simd::SimdBackend;
use stwo_cairo_prover::stwo_prover::core::backend::BackendForChannel;
use stwo_cairo_prover::stwo_prover::core::channel::MerkleChannel;
use stwo_cairo_prover::stwo_prover::core::pcs::PcsConfig;
use stwo_cairo_prover::stwo_prover::core::vcs::blake2_merkle::Blake2sMerkleChannel;
use stwo_cairo_prover::stwo_prover::core::vcs::ops::MerkleHasher;
use stwo_cairo_serialize::CairoSerialize;
use bytemuck::cast_slice;

// Vec-backed writer to capture Cairo encoders' output in-memory.
struct VecWriter<'a> {
    buf: &'a mut Vec<u8>,
    bytes_written: usize,
}
impl<'a> Writer for VecWriter<'a> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.buf.extend_from_slice(bytes);
        self.bytes_written += bytes.len();
        Ok(())
    }
}

pub struct Prover {
    sierra_program: SierraProgram,
}

impl Prover {
    pub fn new() -> Self {
        let raw_json: &[u8] = include_bytes!("../../circuits/target/dev/circuits.sierra.json");
        let sierra_program = serde_json::from_slice(raw_json)
            .expect("circuits.sierra.json must be valid Sierra JSON");
        Self { sierra_program }
    }

    pub fn prove(&self, transactions: &[Transaction], state: &State) -> Result<[u8; 32], Error> {
        // flatten GameState
        let players = encode_players(&state.players_list());

        println!("Players: {:?}", players);
        let nfts = encode_nfts(&state.nfts_list());
        let tx_felts = encode_txs(transactions);

        let player_count = players.len() / 4;
        let nfts_count = nfts.len() / 4;
        let tx_count = tx_felts.len() / 7;

        let mut all: Vec<Felt252> =
            Vec::with_capacity(3 + players.len() + nfts.len() + tx_felts.len());
        all.push(Felt252::from(player_count as u128));
        all.extend(players);
        all.push(Felt252::from(nfts_count as u128));
        all.extend(nfts);
        all.push(Felt252::from(tx_count as u128));
        all.extend(tx_felts);

        let args = vec![FuncArg::Array(all)];

        println!("New root: {:?}", args);

        let cairo_run_config = Cairo1RunConfig {
            args: &args,
            serialize_output: false,
            trace_enabled: true,
            relocate_mem: true,
            layout: LayoutName::all_cairo_stwo,
            proof_mode: true,
            append_return_values: false,
            finalize_builtins: true,
            dynamic_layout_params: None,
        };

        match cairo_run_program(&self.sierra_program, cairo_run_config) {
            Ok((_runner, ret, _serial)) => {
                // Prepare public input in-memory.
                let public_input = _runner.get_air_public_input()?;

                // Encode relocated trace and memory into in-memory buffers.
                let relocated_trace = _runner
                    .relocated_trace
                    .as_ref()
                    .ok_or(Error::Trace(TraceError::TraceNotRelocated))?;

                let mut trace_bytes = Vec::with_capacity(3 * 1024 * 1024);
                cairo_vm::cairo_run::write_encoded_trace(
                    relocated_trace,
                    &mut VecWriter {
                        buf: &mut trace_bytes,
                        bytes_written: 0,
                    },
                )?;

                let mut memory_bytes = Vec::with_capacity(5 * 1024 * 1024);
                cairo_vm::cairo_run::write_encoded_memory(
                    &_runner.relocated_memory,
                    &mut VecWriter {
                        buf: &mut memory_bytes,
                        bytes_written: 0,
                    },
                )?;

                // Reinterpret encoded bytes as typed slices, matching the adapter's file-backed format.
                let trace_entries: &[StwoRelocatedTraceEntry] = cast_slice(&trace_bytes);
                let memory_entries: &[StwoMemoryEntry] = cast_slice(&memory_bytes);

                // Build MemoryBuilder from entries.
                let memory_builder = MemoryBuilder::from_iter(
                    MemoryConfig::default(),
                    memory_entries.iter().copied(),
                );

                // Collect public memory addresses and memory segments.
                let public_memory_addresses: Vec<u32> = public_input
                    .public_memory
                    .iter()
                    .map(|entry| entry.address as u32)
                    .collect();

                let memory_segments: HashMap<&str, MemorySegmentAddresses> = public_input
                    .memory_segments
                    .into_iter()
                    .map(|(k, v)| (k, v.into()))
                    .collect();

                // Create prover input directly.
                let prover_input: ProverInput = adapt_to_stwo_input(
                    trace_entries,
                    memory_builder,
                    public_memory_addresses,
                    &memory_segments,
                    PublicSegmentContext::bootloader_context(),
                ).unwrap();

                // println!("prover_input: {:?}", &prover_input);
                let prover_params = default_prod_prover_parameters();

                let proof_format = ProofFormat::CairoSerde;

                let proof_path = PathBuf::from("./example_proof.json");

                let _cairo_proof = Prover::run_inner::<Blake2sMerkleChannel>(prover_input, prover_params.pcs_config, prover_params.preprocessed_trace, proof_path, proof_format).unwrap();


                let mut it = ret.iter();
                it.next();

                println!("return {:?}", it);
                let players_out = decode_players(&mut it);
                let nfts_out = decode_nfts(&mut it);

                let mut new_state = State::new();
                for p in players_out {
                    new_state.upsert_player(p);
                }
                for n in nfts_out {
                    new_state.upsert_nft(n);
                }
                new_state.commit(); // seals the Merkle tree
                let new_root = new_state.root().expect("new state must have a root");

                /* ---------------------------------------------------
                 * 5.  Debug print – BEFORE vs AFTER
                 * ------------------------------------------------ */
                //println!("--- OLD STATE ---\n{state:#?}");
                //println!("--- NEW STATE ---\n{new_state:#?}");

                Ok(new_root)
            }

            Err(Error::RunPanic(panic_data)) => {
                if !panic_data.is_empty() {
                    let pretty: Vec<String> = panic_data
                        .iter()
                        .map(|felt| {
                            let raw = felt.to_bytes_be();
                            match String::from_utf8(raw.to_vec()) {
                                Ok(txt) => format!("{felt} ('{txt}')"),
                                Err(_) => felt.to_string(),
                            }
                        })
                        .collect();
                    eprintln!("⛔ Cairo panicked: [{}]", pretty.join(", "));
                }
                Err(Error::RunPanic(panic_data))
            }

            Err(err) => {
                eprintln!("🛑 Cairo VM error: {err:?}");
                Err(err)
            }
        }
    }

    pub fn run_inner<MC: MerkleChannel>(
        vm_output: ProverInput,
        pcs_config: PcsConfig,
        preprocessed_trace: PreProcessedTraceVariant,
        proof_path: PathBuf,
        proof_format: ProofFormat,
    ) -> Result<(), Error>
    where
        SimdBackend: BackendForChannel<MC>,
        MC::H: Serialize,
        <MC::H as MerkleHasher>::Hash: CairoSerialize,
    {
        let proof = prove_cairo::<MC>(vm_output, pcs_config, preprocessed_trace).unwrap();
        let mut proof_file = std::fs::File::create(proof_path)?;

        match proof_format {
            ProofFormat::Json => {
                proof_file.write_all(sonic_rs::to_string_pretty(&proof).unwrap().as_bytes())?;
            }
            ProofFormat::CairoSerde => {
                let mut serialized: Vec<starknet_ff::FieldElement> = Vec::new();
                CairoSerialize::serialize(&proof, &mut serialized);

                let hex_strings: Vec<String> = serialized
                    .into_iter()
                    .map(|felt| format!("0x{:x}", felt))
                    .collect();

                proof_file
                    .write_all(sonic_rs::to_string_pretty(&hex_strings).unwrap().as_bytes())?;
            }
        }

        Ok(())
    }
}
