#[macro_use]
extern crate tracing;

mod broker;

use atl_checker::atl::dependencygraph::{ATLDependencyGraph, ATLVertex};
use atl_checker::common::VertexAssignment;
use atl_checker::lcgs::ast::DeclKind;
use atl_checker::lcgs::ir::intermediate::IntermediateLCGS;
use atl_checker::lcgs::ir::symbol_table::Owner;
use atl_checker::lcgs::parse::parse_lcgs;
use std::error::Error;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::console;

// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// This is like the `main` function, except for JavaScript.
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    tracing_wasm::set_as_global_default();

    Ok(())
}

#[wasm_bindgen]
pub fn numbers(model: &str) {
    let lcgs = parse_lcgs(model)
        .map_err(|err| format!("Failed to parse the LCGS program.\n{}", err))
        .unwrap();

    let ir = IntermediateLCGS::create(lcgs)
        .map_err(|err| format!("Invalid LCGS program.\n{}", err))
        .unwrap();

    console::log_1(&JsValue::from_str("Players:"));
    for player in &ir.get_player() {
        console::log_1(&JsValue::from_str(
            format!("{} : {}", player.get_name(), player.index()).as_str(),
        ));
    }

    console::log_1(&JsValue::from_str("\nLabels:"));
    for label_symbol in &ir.get_labels() {
        let label_decl = ir.get_decl(&label_symbol).unwrap();
        if let DeclKind::Label(label) = &label_decl.kind {
            if Owner::Global == label_symbol.owner {
                console::log_1(&JsValue::from_str(
                    format!("{} : {}", &label_symbol.name, label.index).as_str(),
                ));
            } else {
                console::log_1(&JsValue::from_str(
                    format!("{} : {}", &label_symbol, label.index).as_str(),
                ));
            }
        }
    }
}

#[wasm_bindgen]
pub fn check(lcgs_model: &str, atl_formula: &str) -> Result<JsValue, JsValue> {
    let lcgs = parse_lcgs(&lcgs_model)
        .map_err(|err| format!("Failed to parse the LCGS program.\n{}", err))?;

    let game_structure =
        IntermediateLCGS::create(lcgs).map_err(|err| format!("Invalid LCGS program.\n{}", err))?;

    let phi = atl_checker::atl::formula::parse_phi(&game_structure, atl_formula)
        .expect("Invalid ATL formula provided");

    let graph = ATLDependencyGraph { game_structure };

    let v0 = ATLVertex::FULL {
        state: graph.game_structure.initial_state_index(),
        formula: Arc::from(phi),
    };

    let broker = crate::broker::SimpleBroker::new();

    let mut worker = atl_checker::edg::Worker::new(0, 1, v0.clone(), broker.clone(), graph);
    worker.run();

    match broker.get_result() {
        None => panic!("worker terminated without setting result"),
        Some(result) => Ok(JsValue::from(result.to_string())),
    }
}
