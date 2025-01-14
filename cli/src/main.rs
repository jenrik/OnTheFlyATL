#[no_link]
extern crate git_version;
extern crate num_cpus;
#[macro_use]
extern crate tracing;

use std::fmt::Debug;
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::process::exit;
use std::sync::Arc;

use clap::{App, Arg, ArgMatches, SubCommand};
use git_version::git_version;
use tracing::trace;

use atl_checker::atl::dependencygraph::{ATLDependencyGraph, ATLVertex};
use atl_checker::atl::formula::{ATLExpressionParser, Phi};
use atl_checker::atl::gamestructure::{EagerGameStructure, GameStructure};
use atl_checker::edg::distributed_certain_zero;
use atl_checker::lcgs::ast::DeclKind;
use atl_checker::lcgs::ir::intermediate::IntermediateLCGS;
use atl_checker::lcgs::ir::symbol_table::Owner;
use atl_checker::lcgs::parse::parse_lcgs;
#[cfg(feature = "graph-printer")]
use atl_checker::printer::print_graph;
use atl_checker::search_strategy::bfs::BreadthFirstSearchBuilder;
use atl_checker::solve_set::minimum_solve_set;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_VERSION: &str = git_version!(fallback = "unknown");

/// The formula types that the system supports
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum FormulaFormat {
    JSON,
    ATL,
}

/// The model types that the system supports
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum ModelType {
    JSON,
    LCGS,
}

#[tracing::instrument]
fn main() {
    if let Err(msg) = main_inner() {
        println!("{}", msg);
        exit(1);
    }
}

fn main_inner() -> Result<(), String> {
    let args = parse_arguments();

    setup_tracing(&args)?;
    trace!(?args, "commandline arguments");

    match args.subcommand() {
        ("index", Some(index_args)) => {
            // Display the indexes for the players and labels

            let input_model_path = index_args.value_of("input_model").unwrap();
            let model_type = get_model_type_from_args(&index_args)?;

            if model_type != ModelType::LCGS {
                return Err("The 'index' command is only valid for LCGS models".to_string());
            }

            // Open the input model file
            let mut file = File::open(input_model_path)
                .map_err(|err| format!("Failed to open input model.\n{}", err))?;

            // Read the input model from the file into memory
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|err| format!("Failed to read input model.\n{}", err))?;

            let lcgs = parse_lcgs(&content)
                .map_err(|err| format!("Failed to parse the LCGS program.\n{}", err))?;

            let ir = IntermediateLCGS::create(lcgs)
                .map_err(|err| format!("Invalid LCGS program.\n{}", err))?;

            println!("Players:");
            for player in &ir.get_player() {
                println!("{} : {}", player.get_name(), player.index())
            }

            println!("\nLabels:");
            for label_symbol in &ir.get_labels() {
                let label_decl = ir.get_decl(&label_symbol).unwrap();
                if let DeclKind::Label(label) = &label_decl.kind {
                    if Owner::Global == label_symbol.owner {
                        println!("{} : {}", &label_symbol.name, label.index)
                    } else {
                        println!("{} : {}", &label_symbol, label.index)
                    }
                }
            }
        }
        ("solver", Some(solver_args)) => {
            let input_model_path = solver_args.value_of("input_model").unwrap();
            let model_type = get_model_type_from_args(&solver_args)?;
            let formula_path = solver_args.value_of("formula").unwrap();
            let formula_format = get_formula_format_from_args(&solver_args)?;

            // Generic start function for use with `load` that start model checking with `distributed_certain_zero`
            fn check_model<G>(graph: ATLDependencyGraph<G>, v0: ATLVertex, threads: u64)
            where
                G: GameStructure + Send + Sync + Clone + Debug + 'static,
            {
                let result =
                    distributed_certain_zero(graph, v0, threads, BreadthFirstSearchBuilder);
                println!("Result: {}", result);
            }

            let threads = match solver_args.value_of("threads") {
                None => num_cpus::get() as u64,
                Some(t_arg) => t_arg.parse().unwrap(),
            };

            load(
                model_type,
                input_model_path,
                formula_path,
                formula_format,
                |game_structure, formula| {
                    println!(
                        "Checking the formula: {}",
                        formula.in_context_of(&game_structure)
                    );
                    let v0 = ATLVertex::FULL {
                        state: 0,
                        formula: Arc::from(formula),
                    };
                    let graph = ATLDependencyGraph { game_structure };
                    check_model(graph, v0, threads);
                },
                |game_structure, formula| {
                    println!(
                        "Checking the formula: {}",
                        formula.in_context_of(&game_structure)
                    );
                    let arc = Arc::from(formula);
                    let graph = ATLDependencyGraph { game_structure };
                    let v0 = ATLVertex::FULL {
                        state: graph.game_structure.initial_state_index(),
                        formula: arc,
                    };
                    check_model(graph, v0, threads);
                },
            )?
        }
        ("analyse", Some(analyse_args)) => {
            let input_model_path = analyse_args.value_of("input_model").unwrap();
            let model_type = get_model_type_from_args(&analyse_args)?;
            let formula_path = analyse_args.value_of("formula").unwrap();
            let formula_format = get_formula_format_from_args(&analyse_args)?;

            fn analyse_model<G>(graph: ATLDependencyGraph<G>, v0: ATLVertex)
            where
                G: GameStructure + Send + Sync + Clone + Debug + 'static,
            {
                let mss = minimum_solve_set(&graph, v0);
                println!("Configuration: Minimum solve set size");
                for (vertex, assignment) in mss {
                    println!("{}: {}", vertex, assignment.len());
                }
            }

            load(
                model_type,
                input_model_path,
                formula_path,
                formula_format,
                |game_structure, formula| {
                    let v0 = ATLVertex::FULL {
                        state: 0,
                        formula: Arc::from(formula),
                    };
                    let graph = ATLDependencyGraph { game_structure };
                    analyse_model(graph, v0);
                },
                |game_structure, formula| {
                    let v0 = ATLVertex::FULL {
                        state: game_structure.initial_state_index(),
                        formula: Arc::from(formula),
                    };
                    let graph = ATLDependencyGraph { game_structure };
                    analyse_model(graph, v0);
                },
            )?
        }
        ("graph", Some(graph_args)) => {
            #[cfg(feature = "graph-printer")]
            {
                let input_model_path = graph_args.value_of("input_model").unwrap();
                let model_type = get_model_type_from_args(&graph_args)?;
                let formula_path = graph_args.value_of("formula").unwrap();
                let formula_format = get_formula_format_from_args(&graph_args)?;

                // Generic start function for use with `load` that starts the graph printer
                fn print_model<G: GameStructure>(
                    graph: ATLDependencyGraph<G>,
                    v0: ATLVertex,
                    output: Option<&str>,
                ) {
                    let output: Box<dyn Write> = match output {
                        Some(path) => {
                            let file = File::create(path).unwrap_or_else(|err| {
                                eprintln!("Failed to create output file\n\nError:\n{}", err);
                                exit(1);
                            });
                            Box::new(file)
                        }
                        _ => Box::new(stdout()),
                    };

                    print_graph(graph, v0, output).unwrap();
                }

                load(
                    model_type,
                    input_model_path,
                    formula_path,
                    formula_format,
                    |game_structure, formula| {
                        println!(
                            "Printing graph for: {}",
                            formula.in_context_of(&game_structure)
                        );
                        let v0 = ATLVertex::FULL {
                            state: 0,
                            formula: Arc::from(formula),
                        };
                        let graph = ATLDependencyGraph { game_structure };
                        print_model(graph, v0, graph_args.value_of("output"));
                    },
                    |game_structure, formula| {
                        println!(
                            "Printing graph for: {}",
                            formula.in_context_of(&game_structure)
                        );
                        let arc = Arc::from(formula);
                        let graph = ATLDependencyGraph { game_structure };
                        let v0 = ATLVertex::FULL {
                            state: graph.game_structure.initial_state_index(),
                            formula: arc,
                        };
                        print_model(graph, v0, graph_args.value_of("output"));
                    },
                )?
            }
        }
        _ => (),
    };
    Ok(())
}

/// Reads a formula in JSON format from a file and returns the formula as a string
/// and as a parsed Phi struct.
/// This function will exit the program if it encounters an error.
fn load_formula<A: ATLExpressionParser>(path: &str, format: FormulaFormat, expr_parser: &A) -> Phi {
    let mut file = File::open(path).unwrap_or_else(|err| {
        eprintln!("Failed to open formula file\n\nError:\n{}", err);
        exit(1);
    });

    let mut raw_phi = String::new();
    file.read_to_string(&mut raw_phi).unwrap_or_else(|err| {
        eprintln!("Failed to read formula file\n\nError:\n{}", err);
        exit(1);
    });

    match format {
        FormulaFormat::JSON => serde_json::from_str(raw_phi.as_str()).unwrap_or_else(|err| {
            eprintln!("Failed to deserialize formula\n\nError:\n{}", err);
            exit(1);
        }),
        FormulaFormat::ATL => {
            let result = atl_checker::atl::formula::parse_phi(expr_parser, &raw_phi);
            result.unwrap_or_else(|err| {
                eprintln!("Invalid ATL formula provided:\n\n{}", err);
                exit(1)
            })
        }
    }
}

/// Determine the model type (either "json" or "lcgs") by reading the the
/// --model_type argument or inferring it from the model's path extension.  
fn get_model_type_from_args(args: &ArgMatches) -> Result<ModelType, String> {
    match args.value_of("model_type") {
        Some("lcgs") => Ok(ModelType::LCGS),
        Some("json") => Ok(ModelType::JSON),
        None => {
            // Infer model type from file extension
            let model_path = args.value_of("input_model").unwrap();
            if model_path.ends_with(".lcgs") {
                Ok(ModelType::LCGS)
            } else if model_path.ends_with(".json") {
                Ok(ModelType::JSON)
            } else {
                Err("Cannot infer model type from file the extension. You can specify it with '--model_type=MODEL_TYPE'".to_string())
            }
        }
        Some(model_type) => Err(format!("Invalid model type '{}' specified with --model_type. Use either \"lcgs\" or \"json\" [default is inferred from model path].", model_type)),
    }
}

/// Determine the formula format (either "json" or "atl") by reading the
/// --formula_format argument. If none is given, we default to ATL
fn get_formula_format_from_args(args: &ArgMatches) -> Result<FormulaFormat, String> {
    match args.value_of("formula_format") {
        Some("json") => Ok(FormulaFormat::JSON),
        Some("atl") => Ok(FormulaFormat::ATL),
        // Default value in case user did not give one
        None => Ok(FormulaFormat::ATL),
        Some(format) => Err(format!("Invalid formula format '{}' specified with --formula_format. Use either \"atl\" or \"json\" [default is \"atl\"].", format)),
    }
}

/// Loads a model and a formula from files, and then call the handler function with the loaded model and formula.
fn load<R, J, L>(
    model_type: ModelType,
    game_structure_path: &str,
    formula_path: &str,
    formula_format: FormulaFormat,
    handle_json: J,
    handle_lcgs: L,
) -> Result<R, String>
where
    J: FnOnce(EagerGameStructure, Phi) -> R,
    L: FnOnce(IntermediateLCGS, Phi) -> R,
{
    // Open the input model file
    let mut file = File::open(game_structure_path)
        .map_err(|err| format!("Failed to open input model.\n{}", err))?;
    // Read the input model from the file into memory
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|err| format!("Failed to read input model.\n{}", err))?;

    // Depending on which model_type is specified, use the relevant parsing logic
    match model_type {
        ModelType::JSON => {
            let game_structure = serde_json::from_str(content.as_str())
                .map_err(|err| format!("Failed to deserialize input model.\n{}", err))?;

            let phi = load_formula(formula_path, formula_format, &game_structure);

            Ok(handle_json(game_structure, phi))
        }
        ModelType::LCGS => {
            let lcgs = parse_lcgs(&content)
                .map_err(|err| format!("Failed to parse the LCGS program.\n{}", err))?;

            let game_structure = IntermediateLCGS::create(lcgs)
                .map_err(|err| format!("Invalid LCGS program.\n{}", err))?;

            let phi = load_formula(formula_path, formula_format, &game_structure);

            Ok(handle_lcgs(game_structure, phi))
        }
    }
}

/// Define and parse command line arguments
fn parse_arguments() -> ArgMatches<'static> {
    fn build_common_arguments<'a>(builder: clap::App<'a, 'a>) -> App<'a, 'a> {
        builder
            .arg(
                Arg::with_name("input_model")
                    .short("m")
                    .long("model")
                    .env("INPUT_MODEL")
                    .required(true)
                    .help("The input file to generate model from"),
            )
            .arg(
                Arg::with_name("model_type")
                    .short("t")
                    .long("model-type")
                    .env("MODEL_TYPE")
                    .help("The type of input file given {{lcgs, json}}"),
            )
            .arg(
                Arg::with_name("formula_format")
                    .short("y")
                    .long("formula-format")
                    .env("FORMULA_FORMAT")
                    .help("The format of ATL formula file given {{json, text}}"),
            )
            .arg(
                Arg::with_name("formula")
                    .short("f")
                    .long("formula")
                    .env("FORMULA")
                    .required(true)
                    .help("The formula to check for"),
            )
            .arg(
                Arg::with_name("output")
                    .short("o")
                    .long("output")
                    .env("OUTPUT")
                    .help("The path to write output to"),
            )
    }

    let version_text = format!("{} ({})", VERSION, GIT_VERSION);
    let app = App::new(PKG_NAME)
        .version(version_text.as_str())
        .author(AUTHORS)
        .arg(
            Arg::with_name("log_filter")
                .short("l")
                .long("log-filter")
                .env("RUST_LOG")
                .default_value("warn")
                .help("Comma separated list of filter directives"),
        )
        .subcommand(build_common_arguments(
            SubCommand::with_name("solver").arg(
                Arg::with_name("threads")
                    .short("r")
                    .long("threads")
                    .env("THREADS")
                    .help("Number of threads to run solver on"),
            ),
        ))
        .subcommand(
            SubCommand::with_name("index").arg(
                Arg::with_name("input_model")
                    .short("m")
                    .long("model")
                    .env("INPUT_MODEL")
                    .required(true)
                    .help("The input file to generate model from"),
            ),
        )
        .subcommand(build_common_arguments(SubCommand::with_name("analyse")));

    if cfg!(feature = "graph-printer") {
        app.subcommand(build_common_arguments(SubCommand::with_name("graph")))
            .get_matches()
    } else {
        app.get_matches()
    }
}

fn setup_tracing(args: &ArgMatches) -> Result<(), String> {
    // Configure a filter for tracing data if one have been set
    if let Some(filter) = args.value_of("log_filter") {
        let filter = tracing_subscriber::EnvFilter::try_new(filter)
            .map_err(|err| format!("Invalid log filter.\n{}", err))?;
        tracing_subscriber::fmt().with_env_filter(filter).init()
    } else {
        tracing_subscriber::fmt().init()
    }
    Ok(())
}
