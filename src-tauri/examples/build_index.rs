//! Usage: build_index <tracker.json> <credit_names.txt> <sha>  (writes index JSON to stdout)
use wanderdex_lib::index;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: build_index <tracker.json> <credit_names.txt> <sha>");
        std::process::exit(2);
    }
    let tracker: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[1]).expect("read tracker.json")).expect("parse tracker.json");
    let credit_names = std::fs::read_to_string(&args[2]).expect("read credit_names.txt");
    let idx = index::build(&tracker, &credit_names, &args[3]);
    let variants: usize = idx.species.iter().map(|s| s.variants.len()).sum();
    eprintln!("{} species, {} variants", idx.species.len(), variants);
    println!("{}", serde_json::to_string(&idx).expect("serialize index"));
}
