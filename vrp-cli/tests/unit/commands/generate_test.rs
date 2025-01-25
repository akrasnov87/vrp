use super::*;
use crate::cli::{get_app, run_subcommand};
use vrp_cli::pragmatic::format::problem::PragmaticProblem;

const PRAGMATIC_PROBLEM_PATH: &str = "../examples/data/pragmatic/simple.basic.problem.json";

#[test]
fn can_generate_problem_from_args() {
    let tmpfile = tempfile::NamedTempFile::new().unwrap();
    let args = vec![
        "vrp-cli",
        "generate",
        "pragmatic",
        "--prototypes",
        PRAGMATIC_PROBLEM_PATH,
        "--jobs-size",
        "100",
        "--vehicles-size",
        "10",
        "--out-result",
        tmpfile.path().to_str().unwrap(),
    ];
    let matches = get_app().try_get_matches_from(args).unwrap();

    run_subcommand(matches);

    let problem = BufReader::new(tmpfile.as_file()).read_pragmatic().unwrap();
    assert_eq!(problem.jobs.size(), 100);
    assert_eq!(problem.fleet.vehicles.len(), 10);
}
