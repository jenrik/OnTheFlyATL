#[macro_use]
extern crate tracing;

mod broker;

use atl_checker::atl::dependencygraph::{ATLDependencyGraph, ATLVertex};
use atl_checker::common::VertexAssignment;
use atl_checker::lcgs::ast::DeclKind;
use atl_checker::lcgs::ir::intermediate::IntermediateLCGS;
use atl_checker::lcgs::ir::symbol_table::Owner;
use atl_checker::lcgs::parse::parse_lcgs;
use atl_checker::search_strategy::bfs::BreadthFirstSearchBuilder;
use atl_checker::search_strategy::SearchStrategyBuilder;
use std::error::Error;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, HtmlButtonElement, HtmlDivElement, HtmlInputElement, HtmlTextAreaElement};

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

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");

    let elem_lcgs = document
        .get_element_by_id("lcgs-model")
        .unwrap()
        .dyn_into::<HtmlTextAreaElement>()
        .unwrap();
    let elem_atl = document
        .get_element_by_id("atl-formula")
        .unwrap()
        .dyn_into::<HtmlInputElement>()
        .unwrap();
    let elem_submit = document
        .get_element_by_id("solve")
        .unwrap()
        .dyn_into::<HtmlButtonElement>()
        .unwrap();
    let elem_result = document
        .get_element_by_id("result")
        .unwrap()
        .dyn_into::<HtmlDivElement>()
        .unwrap();

    let a = Closure::wrap(Box::new(move || {
        let lcgs: String = elem_lcgs.value().to_string();
        let atl: String = elem_atl.value().to_string();
        let result = check_model(lcgs.as_str(), atl.as_str());
        elem_result.set_inner_html(format!("{:?}", result.unwrap()).as_str());
    }) as Box<dyn FnMut() + 'static>);
    elem_submit.set_onclick(Some(a.as_ref().unchecked_ref()));
    a.forget();

    Ok(())
}

fn check_model(lcgs_model: &str, atl_formula: &str) -> Result<VertexAssignment, Box<dyn Error>> {
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

    let mut worker = atl_checker::edg::Worker::new(
        0,
        1,
        v0.clone(),
        broker.clone(),
        graph,
        BreadthFirstSearchBuilder.build(),
    );
    worker.run();

    match broker.get_result() {
        None => panic!("worker terminated without setting result"),
        Some(result) => Ok(result),
    }
}
