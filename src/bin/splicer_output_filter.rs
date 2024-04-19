use std::path::{Path, PathBuf};

use clap::{ArgAction, Parser};
use tqdm::tqdm;
use walkdir::WalkDir;

use icemaker::fuzz_tree_splicer::IN_CODE_FP_KEYWORDS;

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    check_file: Option<String>,
    #[clap(long)]
    check_dir: Option<String>,
    #[clap(long, action=ArgAction::SetTrue)]
    print_bads: bool,
}

pub fn check_fp_keywords(p: &PathBuf) -> bool {
    let content = std::fs::read_to_string(p).unwrap_or_default();
    !IN_CODE_FP_KEYWORDS
        .iter()
        .any(|fp_keyword| content.contains(fp_keyword))
}

pub fn main() {
    let args = Cli::parse();

    if let Some(filename) = args.check_file {
        let path = std::path::PathBuf::from(filename);
        if check_fp_keywords(&path) {
            println!("good");
        } else {
            println!("bad");
        }
    } else if let Some(dirname) = args.check_dir {
        let dir = Path::new(&dirname);
        for entry in tqdm(WalkDir::new(dir).into_iter()).style(tqdm::Style::Block) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if path.is_file() && ext.to_string_lossy() == "rs" {
                        match (check_fp_keywords(&path.to_path_buf()), args.print_bads) {
                            (true, false) => println!("{}", path.canonicalize().unwrap().display()),
                            (false, true) => println!("{}", path.canonicalize().unwrap().display()),
                            _ => (),
                        }
                    }
                }
            }
        }
    }
}
