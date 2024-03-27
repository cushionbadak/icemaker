use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tqdm::tqdm;
use tree_sitter::Parser;
use walkdir::WalkDir;

// from fuzz_tree_splicer.rs
// such files will most likely just causes known crashes or hang the splicing
pub(crate) fn ignore_file_for_splicing(file: &PathBuf) -> bool {
    const LINE_LIMIT: usize = 400; // was 1000

    let content = std::fs::read_to_string(file).unwrap_or_default();
    let lines_count = content.lines().count();

    lines_count > LINE_LIMIT
        || content.contains("no_core")
        || content.contains("lang_items")
        || content.contains("mir!(")
        || content.contains("break rust")
        || (content.contains("failure-status: 101") && content.contains("known-bug"))
        // if the file is in an "icemaker" dir, do not use it for fuzzing...
        || file.display().to_string().contains("icemaker")
}

// from flags.rs
pub(crate) static EXCEPTIONS: &[&str] = &[
    // runtime
    "tests/ui/closures/issue-72408-nested-closures-exponential.rs",
    "tests/ui/issues/issue-74564-if-expr-stack-overflow.rs",
    "library/stdarch/crates/core_arch/src/mod.rs", //10+ mins
    // memory
    "tests/ui/issues/issue-50811.rs",
    "tests/ui/issues/issue-29466.rs",
    "src/tools/miri/tests/run-pass/float.rs",
    "tests/ui/numbers-arithmetic/saturating-float-casts-wasm.rs",
    "tests/ui/numbers-arithmetic/saturating-float-casts-impl.rs",
    "tests/ui/numbers-arithmetic/saturating-float-casts.rs",
    "tests/ui/wrapping-int-combinations.rs",
    // glacier/memory/time:
    "fixed/23600.rs",
    "23600.rs",
    "fixed/71699.rs",
    "71699.rs",
    // runtime
    "library/stdarch/crates/core_arch/src/x86/avx512bw.rs",
    "library/stdarch/crates/core_arch/src/x86/mod.rs",
    // 3.5 hours when reporting errors :(
    "library/stdarch/crates/core_arch/src/lib.rs",
    // memory 2.0
    "tests/run-make-fulldeps/issue-47551/eh_frame-terminator.rs",
    // infinite recursion in rustdoc, can take tens of minutes in ci
    "tests/ui/recursion/issue-38591-non-regular-dropck-recursion.rs",
    "tests/ui/dropck/dropck_no_diverge_on_nonregular_2.rs",
    "tests/ui/dropck/dropck_no_diverge_on_nonregular_1.rs",
    // 900 mb output, can take 5-10 minutes
    "tests/run-make-fulldeps/issue-47551/eh_frame-terminator.rs",
    // very slow
    "library/stdarch/crates/core_arch/src/x86/mod.rs",
    "library/core/src/lib.rs",
    "library/stdarch/crates/core_arch/src/mod.rs",
    "compiler/rustc_middle/src/lib.rs",
    "library/stdarch/crates/core_arch/src/x86/avx512f.rs",
    "tests/ui/structs-enums/struct-rec/issue-84611.rs",
    "tests/ui/structs-enums/struct-rec/issue-74224.rs",
    "tests/ui/dropck/dropck_no_diverge_on_nonregular_3.rs",
    "library/portable-simd/crates/core_simd/src/lib.rs", // 12+ minutes
    "tests/ui-fulldeps/myriad-closures.rs",
    "src/tools/miri/tests/pass/float.rs",
    "library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs",
    "library/stdarch/crates/core_arch/src/aarch64/mod.rs",
    "library/stdarch/crates/core_arch/src/aarch64/neon/generated.rs",
    "library/stdarch/crates/core_arch/src/aarch64/neon/mod.rs",
    "src/tools/cargo/tests/testsuite/main.rs",
    "src/tools/clippy/clippy_lints/src/lib.rs",
    "library/stdarch/crates/stdarch-gen/src/main.rs",
    "src/tools/rust-analyzer/crates/proc-macro-srv/src/abis/abi_1_58/proc_macro/mod.rs",
    "src/tools/rust-analyzer/crates/proc-macro-srv/src/abis/abi_1_63/proc_macro/mod.rs",
    "tests/ui/issues/issue-22638.rs",
    "tests/ui/issues/issue-72933-match-stack-overflow.rs",
    "tests/ui/recursion/issue-86784.rs",
    "tests/ui/associated-types/issue-67684.rs",
];

fn get_exception_list(root_path: &PathBuf) -> Vec<PathBuf> {
    EXCEPTIONS
        .iter()
        .map(PathBuf::from)
        .map(|p| root_path.join(p))
        .collect()
}

// from main.rs fn codegen_tree_splicer_omni
// seed_able_files() returns a list of files that can be used as seed files for the fuzzer,
fn _seed_able_files() -> Vec<String> {
    let root_path = std::env::current_dir().expect("no cwd!");

    eprintln!("collecting files..");
    // files we use as dataset
    let files = WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        // skip any paths that contain "icemaker" because this was probably already generated for fuzzing, no need to use these as input for more fuzzing
        .filter(|f| !format!("{:?}", f).contains("icemaker"))
        .filter(|f| !f.path().display().to_string().contains(".git"))
        .map(|f| f.path().to_owned())
        .filter(|pb| !ignore_file_for_splicing(pb))
        .collect::<Vec<PathBuf>>();

    let mut parser = Parser::new();
    // rust!
    parser.set_language(tree_sitter_rust::language()).unwrap();

    eprintln!("parsing {} files..", files.len());

    tqdm(files.iter())
        .style(tqdm::Style::Block)
        .map(|p| (p, std::fs::read_to_string(p).unwrap_or_default()))
        .filter_map(|(p, file_content)| {
            parser
                .parse(&file_content, None)
                .map(|_| p.display().to_string())
        })
        .collect::<Vec<String>>()
}

fn seed_able_files_partition() -> (Vec<String>, Vec<String>) {
    let root_path = std::env::current_dir().expect("Failed to get current directory");

    let exception_list = get_exception_list(&root_path);

    eprintln!("Collecting Rust source files...");

    let files = tqdm(WalkDir::new(root_path).into_iter())
        .style(tqdm::Style::Block)
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        // skip any paths that contain "icemaker" because this was probably already generated for fuzzing, no need to use these as input for more fuzzing
        .filter(|f| !format!("{:?}", f).contains("icemaker"))
        .filter(|f| !f.path().display().to_string().contains(".git"))
        .map(|f| f.path().to_owned())
        .filter(|pb| !ignore_file_for_splicing(pb))
        .filter(|pb| !exception_list.contains(pb))
        .collect::<Vec<PathBuf>>();

    let mut parser = Parser::new();
    parser
        .set_language(tree_sitter_rust::language())
        .expect("Failed to set language to Rust");

    eprintln!("Parsing {} files...", files.len());

    let (satisfying, unsatisfying): (Vec<_>, Vec<_>) = tqdm(files.into_iter())
        .style(tqdm::Style::Block)
        .map(|p| {
            let content = std::fs::read_to_string(&p);
            (p, content)
        })
        .partition(|(_, file_content_res)| match file_content_res {
            Ok(file_content) => parser.parse(&file_content, None).is_some(),
            Err(_) => false,
        });

    let satisfying = satisfying
        .into_iter()
        .map(|(p, _)| p.display().to_string())
        .collect();
    let unsatisfying = unsatisfying
        .into_iter()
        .map(|(p, _)| p.display().to_string())
        .collect();

    (satisfying, unsatisfying)
}

fn main() {
    let (satisfying, unsatisfying) = seed_able_files_partition();

    eprintln!("Found {} files satisfying the condition.", satisfying.len());
    eprintln!(
        "Found {} files not satisfying the condition.",
        unsatisfying.len()
    );

    //if !unsatisfying.is_empty() {
    //    _prompt_for_removal(unsatisfying);
    //}

    // if argument "-print-sat" is passed, print the list of satisfying files
    if std::env::args().any(|arg| arg == "-print-sat") {
        for file in &satisfying {
            println!("{}", file);
        }
    }
}

fn _prompt_for_removal(files_to_remove: Vec<String>) {
    // Display up to five file paths
    let preview_files = files_to_remove.iter().take(5);
    eprintln!(
        "\nThe following files did not satisfy the conditions and will be deleted if you proceed:"
    );
    for file in preview_files {
        eprintln!("  - {}", file);
    }
    if files_to_remove.len() > 5 {
        eprintln!("  ...and {} more files.", files_to_remove.len() - 5);
    }

    eprintln!(
        "\nDo you want to proceed with removing these {} file(s)? [yes/NO]: ",
        files_to_remove.len()
    );
    let mut input = String::new();
    io::stdout().flush().expect("Failed to flush stdout");
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");

    if input.trim().eq_ignore_ascii_case("yes") {
        for file_path in files_to_remove {
            if let Err(e) = fs::remove_file(Path::new(&file_path)) {
                eprintln!("Failed to delete {}: {}", file_path, e);
            } else {
                eprintln!("Successfully deleted {}", file_path);
            }
        }
    }
}
