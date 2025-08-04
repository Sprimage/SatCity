use crate::helpers::{decode_nfts, decode_players, encode_nfts, encode_players, encode_txs};
use crate::mempool::Transaction;
use crate::state::State;
use bincode::enc::write::Writer;
use cairo1_run::error::Error;
use cairo1_run::{cairo_run_program, Cairo1RunConfig, FuncArg};
use cairo_air::utils::{ ProofFormat};
use cairo_air::PreProcessedTraceVariant;
use cairo_lang_sierra::program::Program as SierraProgram;
use cairo_vm::types::layout_name::LayoutName;
use cairo_vm::Felt252;
use cairo_vm::{air_public_input::PublicInputError, vm::errors::trace_errors::TraceError};
use serde::Serialize;
use std::io::{self, Write};
use std::path::Path;
use std::path::PathBuf;
use stwo_cairo_adapter::vm_import::{ adapt_vm_output};
use stwo_cairo_adapter::ProverInput;
use stwo_cairo_prover::prover::{default_prod_prover_parameters, prove_cairo};
use stwo_cairo_prover::stwo_prover::core::backend::simd::SimdBackend;
use stwo_cairo_prover::stwo_prover::core::backend::BackendForChannel;
use stwo_cairo_prover::stwo_prover::core::channel::MerkleChannel;
use stwo_cairo_prover::stwo_prover::core::pcs::PcsConfig;
use stwo_cairo_prover::stwo_prover::core::vcs::blake2_merkle::Blake2sMerkleChannel;
use stwo_cairo_prover::stwo_prover::core::vcs::ops::MerkleHasher;
use stwo_cairo_serialize::CairoSerialize;

pub struct FileWriter {
    buf_writer: io::BufWriter<std::fs::File>,
    bytes_written: usize,
}

impl Writer for FileWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.buf_writer
            .write_all(bytes)
            .map_err(|e| bincode::error::EncodeError::Io {
                inner: e,
                index: self.bytes_written,
            })?;

        self.bytes_written += bytes.len();

        Ok(())
    }
}

impl FileWriter {
    fn new(buf_writer: io::BufWriter<std::fs::File>) -> Self {
        Self {
            buf_writer,
            bytes_written: 0,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf_writer.flush()
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
                let public_input_path = Path::new("./public_inputs.json");
                let private_input_path = Path::new("./private_inputs.json");
                let trace_file = Path::new("./trace/trace.bin");
                let memory_file = Path::new("./trace/memory.bin");

                let trace_path = trace_file
                    .canonicalize()
                    .unwrap_or(trace_file.to_path_buf())
                    .to_string_lossy()
                    .to_string();
                let memory_path = memory_file
                    .canonicalize()
                    .unwrap_or(memory_file.to_path_buf())
                    .to_string_lossy()
                    .to_string();

                let public_inputs_json = _runner.get_air_public_input()?.serialize_json()?;
                std::fs::write(public_input_path, public_inputs_json)?;

                let private_inputs_json = _runner
                    .get_air_private_input()
                    .to_serializable(trace_path, memory_path)
                    .serialize_json()
                    .map_err(PublicInputError::Serde)?;
                std::fs::write(private_input_path, private_inputs_json)?;

                let relocated_trace = _runner
                    .relocated_trace
                    .ok_or(Error::Trace(TraceError::TraceNotRelocated))?;
                let trace_file_created = std::fs::File::create(trace_file)?;
                let mut trace_writer = FileWriter::new(io::BufWriter::with_capacity(
                    3 * 1024 * 1024,
                    trace_file_created,
                ));
                cairo_vm::cairo_run::write_encoded_trace(&relocated_trace, &mut trace_writer)?;
                trace_writer.flush()?;

                let memory_file_created = std::fs::File::create(memory_file)?;
                let mut memory_writer = FileWriter::new(io::BufWriter::with_capacity(
                    5 * 1024 * 1024,
                    memory_file_created,
                ));
                cairo_vm::cairo_run::write_encoded_memory(
                    &_runner.relocated_memory,
                    &mut memory_writer,
                )?;
                memory_writer.flush()?;

                let prover_input: ProverInput = adapt_vm_output(public_input_path, private_input_path).expect("");

                // println!("prover_input: {:?}", &prover_input);
                let prover_params = default_prod_prover_parameters();

                let proof_format = ProofFormat::CairoSerde;

                let proof_path = Path::new("./example_proof.json").to_path_buf();

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
