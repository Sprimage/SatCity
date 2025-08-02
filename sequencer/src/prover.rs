use crate::mempool::Transaction;
use std::path::Path;
use crate::state::State;
use crate::helpers::{encode_players, encode_txs,  encode_nfts, decode_players, decode_nfts};
use cairo1_run::error::Error;
use cairo1_run::{cairo_run_program, Cairo1RunConfig, FuncArg };
use cairo_vm::types::layout_name::LayoutName;
use cairo_vm::Felt252;
use cairo_lang_sierra::{
    program::{Program as SierraProgram}
};
use stwo_cairo_adapter::adapter::adapter;
use stwo_cairo_prover::stwo_prover::core::pcs::PcsConfig;
use stwo_cairo_prover::stwo_prover::core::fri::FriConfig;
use cairo_prove::prove::{prove};
use stwo_cairo_adapter::ProverInput;
use stwo_cairo_prover::stwo_prover::core::vcs::blake2_merkle::{
    Blake2sMerkleChannel
};
use cairo_air::utils::{ProofFormat, serialize_proof_to_file};


fn secure_pcs_config() -> PcsConfig {
    PcsConfig {
        pow_bits: 26,
        fri_config: FriConfig {
            log_last_layer_degree_bound: 0,
            log_blowup_factor: 1,
            n_queries: 70,
        },
    }
}

pub struct Prover {
    sierra_program: SierraProgram,
}

impl Prover {
    pub fn new() -> Self {
        let raw_json: &[u8] = include_bytes!("../../circuits/target/dev/circuits.sierra.json");
        let sierra_program = serde_json::from_slice(raw_json).expect("circuits.sierra.json must be valid Sierra JSON");
        Self {
            sierra_program,
        }
    }

    pub fn prove(
        &self,
        transactions: &[Transaction],
        state: &State
    ) -> Result<[u8; 32], Error> {
        // flatten GameState
        let players = encode_players(&state.players_list());

        println!("Players: {:?}", players);
        let nfts    = encode_nfts(&state.nfts_list());
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
            serialize_output: true,
            trace_enabled: true,
            relocate_mem: true,
            layout: LayoutName::all_cairo_stwo,
            proof_mode: true,
            append_return_values: true,
            finalize_builtins: true,
            dynamic_layout_params: None,
        };

        match cairo_run_program(&self.sierra_program, cairo_run_config) {
            Ok((_runner, ret, _serial)) => {

                let mut prover_input_info = _runner.get_prover_input_info().expect("Failed to get prover input");
                let prover_input: ProverInput = adapter(&mut prover_input_info).expect("Failed to run adapter");

                let pcs_config: PcsConfig = secure_pcs_config();

                let cairo_proof = prove(prover_input, pcs_config);
                let proof_format = ProofFormat::CairoSerde;

                let proof_path = Path::new("./example_proof.json");

                serialize_proof_to_file::<Blake2sMerkleChannel>(&cairo_proof, proof_path.into(), proof_format)
                    .expect("Failed to serialize proof");

                let mut it = ret.iter();
                it.next();

                println!("return {:?}", it);
                let players_out = decode_players(&mut it);
                let nfts_out    = decode_nfts(&mut it);

                let mut new_state = State::new();
                for p in players_out { new_state.upsert_player(p); }
                for n in nfts_out   { new_state.upsert_nft(n);     }
                new_state.commit();                       // seals the Merkle tree
                let new_root = new_state
                    .root()
                    .expect("new state must have a root");

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
                                Err(_)  => felt.to_string(),
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
}

