use crate::mempool::Transaction;
use crate::state::State;
use crate::helpers::{encode_players, encode_txs,  encode_nfts};
use cairo1_run::error::Error;
use cairo1_run::{cairo_run_program, Cairo1RunConfig, FuncArg };
use cairo_vm::types::layout_name::LayoutName;
use cairo_vm::types::relocatable::MaybeRelocatable;
use cairo_vm::Felt252;
use cairo_lang_sierra::{
    program::{Program as SierraProgram}
};


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
            layout: LayoutName::all_cairo,
            proof_mode: true,
            append_return_values: true,
            finalize_builtins: true,
            dynamic_layout_params: None,
        };

        match cairo_run_program(&self.sierra_program, cairo_run_config) {
            Ok((_runner, ret, _serial)) => {
                let lo = as_felt(&ret[0]).to_bytes_le();
                let hi = as_felt(&ret[1]).to_bytes_le();
                let mut root = [0u8; 32];
                root[..16].copy_from_slice(&lo[..16]);
                root[16..].copy_from_slice(&hi[..16]);
                Ok(root)
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

fn as_felt(value: &MaybeRelocatable) -> Felt252 {
    match value {
        MaybeRelocatable::Int(f) => *f,
        _ => panic!("expected an integer felt, got relocatable"),
    }
}