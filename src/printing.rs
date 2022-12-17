use std::io::Write;
use std::sync::RwLock;

use crate::ice::*;

#[derive(Eq, PartialEq, Debug, Clone)]
pub(crate) enum PrintMessage {
    Progress {
        index: usize,
        total_number_of_files: usize,
        file_name: String,
    },
    IceFound {
        ice: ICEDisplay,
    },
}

pub(crate) struct Printer {
    prev: RwLock<PrintMessage>,
}

impl Printer {
    pub(crate) fn log(&self, new: PrintMessage) {
        let prev = self.prev.read().unwrap().clone();

        if new == prev {
            // no new message, nothing to update
            return;
        }

        match (prev, &new) {
            // displays "%perc Checking $file ..."
            (
                PrintMessage::Progress { .. },
                PrintMessage::Progress {
                    index,
                    total_number_of_files,
                    file_name,
                },
            ) => {
                let perc = ((index * 100) as f32 / *total_number_of_files as f32) as u8;

                // do not print a newline so we can (\r-eturn carry) our next status update to the same line, requires flushing though
                // we actually need to print a number of space at the end to "clear" remains of previous lines if previous filename was much longer
                print!("\r[{index}/{total_number_of_files} {perc}%] Checking {file_name: <150}",);
                let _stdout = std::io::stdout().flush();
                // kinda ignore whether this fails or not
            }
            (PrintMessage::IceFound { .. }, PrintMessage::IceFound { ref ice }) => {
                println!("{ice}");
            }
            (PrintMessage::Progress { .. }, PrintMessage::IceFound { ref ice }) => {
                println!("\r{ice}");
            }
            (
                PrintMessage::IceFound { .. },
                PrintMessage::Progress {
                    index,
                    total_number_of_files,
                    file_name,
                },
            ) => {
                // let _stdout = std::io::stdout().flush();

                let perc = ((index * 100) as f32 / *total_number_of_files as f32) as u8;

                // do not print a newline so we can (\r-eturn carry) our next status update to the same line, requires flushing though
                // we actually need to print a number of space at the end to "clear" remains of previous lines if previous filename was much longer
                print!("[{index}/{total_number_of_files} {perc}%] Checking {file_name: <150}",);
                let _stdout = std::io::stdout().flush();
            }
        }

        // save new message
        // try_write() will panic if it can't (instead of hang)
        // TODO handle this
        let mut w = self.prev.try_write().unwrap();
        *w = new;
    }

    pub(crate) const fn new() -> Self {
        Printer {
            prev: RwLock::new(PrintMessage::Progress {
                index: 0,
                total_number_of_files: 0,
                file_name: String::new(),
            }),
        }
    }
}
