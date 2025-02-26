use std::io::{self, BufRead, BufWriter};
use std::env;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::io::Write;

use lscolors::{LsColors, Style};

#[cfg(all(
    not(feature = "nu-ansi-term"),
))]
compile_error!(
    "feature must be enabled: nu-ansi-term"
);

fn print_lscolor_path(handle: &mut dyn Write, ls_colors: &LsColors, path: &str) -> io::Result<()> {
    for (component, style) in ls_colors.style_for_path_components(Path::new(path)) {
        #[cfg(any(feature = "nu-ansi-term", feature = "gnu_legacy"))]
        {
            let ansi_style = style.map(Style::to_nu_ansi_term_style).unwrap_or_default();
            write!(handle, "{}", ansi_style.paint(component.to_string_lossy()))?;
        }
    }
    Ok(())
}

/*
 Example output of git diff --format= --summary HASH
  create mode 100644 jdk/test/security/infra/java/security/cert/CertPathValidator/certification/CAInterop.java
  delete mode 100644 jdk/test/security/infra/java/security/cert/CertPathValidator/certification/ComodoCA.java
  rename jdk/test/security/infra/java/security/cert/CertPathValidator/certification/{CertignaRoots.java => CertignaCA.java} (73%)
  rename install/src/macosx/au/Sparkle/{ => Autoupdate}/SUInstaller.m (51%)
  rename jdk/test/{closed => }/java/awt/datatransfer/CRLFTest/CRLFTest.java (53%)
  rename test.txt => test_wow.txt (100%)
*/

// The output is wrapped in a Result to allow matching on errors.
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

const T_CREATE: &str = "create mode ";
const T_DELETE: &str = "delete mode ";
const T_RENAME: &str = "rename ";

const LARROW: &str = "←";
const RARROW: &str = "→";
const SQUARE: &str = "▪";
const CIRCLE: &str = "●";

fn main() {
    let ls_colors = LsColors::from_env().unwrap_or_default();
    let mut stdout = io::stdout();

    let summary_in;
    let summary_out;

    let args: Vec<_> = env::args().collect();
    if args.len() == 3 {
        summary_in = &args[1];
        summary_out = &args[2];
    } else {
        return;
    }

    let file_out;
    if let Ok(file) = File::create(summary_out) {
        file_out = file;
    } else {
        return;
    }
    let mut writer = BufWriter::new(&file_out);

    let mut create_map: HashMap<String, String> = HashMap::new();
    let mut delete_map: HashMap<String, String> = HashMap::new();
    let mut from_map: HashMap<String, String> = HashMap::new();
    let mut to_map: HashMap<String, String> = HashMap::new();
    let mut percent_map: HashMap<String, String> = HashMap::new();

    if let Ok(lines) = read_lines(summary_in) {
        for line in lines.flatten() {
            let mut ln = line.trim();
            if ln.starts_with(T_CREATE) {
                // consume header
                ln = &ln[T_CREATE.len()..];
                // consume mode numbers
                if let Some(idx) = ln.find(" ") {
                    let filename = &ln[idx+1..];
                    // println!("CREATE: |{}|", filename);
                    create_map.insert(filename.to_string(), "true".to_string());
                }
            } else if ln.starts_with(T_DELETE) {
                // consume header
                ln = &ln[T_DELETE.len()..];
                // consume mode numbers
                if let Some(idx) = ln.find(" ") {
                    let filename = &ln[idx+1..];
                    // println!("DELETE: |{}|", filename);
                    delete_map.insert(filename.to_string(), "true".to_string());
                }
            } else if ln.starts_with(T_RENAME) {
                // consume header
                ln = &ln[T_RENAME.len()..];

                // parse percent part first
                let mut percent = "";
                let p1 = ln.rfind(" (");
                let p2 = ln.rfind(")");
                if let (Some(pstart), Some(pend)) = (p1, p2) {
                    if pend > pstart {
                        percent = &ln[pstart+2..pend];
                        // println!("percent={}", percent);
                    }
                    // remove the percent part from the ln
                    ln = &ln[..pstart];
                    // println!("line=|{}|", ln);
                }

                // parse rename pattern
                let lb = ln.find("{");
                let rb = ln.find("}");
                let sp = ln.find(" => ");
                if let (Some(lbracket), Some(rbracket), Some(split)) = (lb, rb, sp) {
                    let prefix = &ln[..lbracket];
                    let from = &ln[lbracket+1..split];
                    let to = &ln[split+4..rbracket];
                    let postfix = &ln[rbracket+1..];
                    let mut path_from = format!("{}{}{}", prefix, from, postfix);
                    let mut path_to = format!("{}{}{}", prefix, to, postfix);
                    path_from = path_from.replace("//", "/"); // fix for empty from
                    path_to = path_to.replace("//", "/");     // fix for empty to
                    // println!("PATH_FROM=|{}|", path_from);
                    // println!("PATH_TO=  |{}|", path_to);

                    to_map.insert(path_from.to_string(), path_to.to_string());
                    from_map.insert(path_to.to_string(), path_from.to_string());
                    if percent.len() > 0 {
                        percent_map.insert(path_from.to_string(), percent.to_string());
                        percent_map.insert(path_to.to_string(), percent.to_string());
                    }

                    // Populate the rename entry file for preview_git_show.sh/git_show.sh
                    // Note that colon(:) is actuqlly allowed characer in linux filesystem,
                    // however I believe "::" is not a common pattern in filename. So use it as delimiter.
                    let buff = format!("{}::{}::{}\n", path_from, path_to, percent);
                    let _ = writer.write(buff.as_bytes());
                } else if let Some(split) = sp {
                    let path_from = &ln[..split];
                    let path_to = &ln[split+4..];
                    // println!("FROM=|{}| TO=|{}|", path_from, path_to);

                    to_map.insert(path_from.to_string(), path_to.to_string());
                    from_map.insert(path_to.to_string(), path_from.to_string());
                    if percent.len() > 0 {
                        percent_map.insert(path_from.to_string(), percent.to_string());
                        percent_map.insert(path_to.to_string(), percent.to_string());
                    }

                    // Populate the rename entry file for preview_git_show.sh/git_show.sh
                    // Note that colon(:) is actuqlly allowed characer in linux filesystem,
                    // however I believe "::" is not a common pattern in filename. So use it as delimiter.
                    let buff = format!("{}::{}::{}\n", path_from, path_to, percent);
                    let _ = writer.write(buff.as_bytes());
                }
            }
        }
    } else {
        return;
    }

    let stdin = io::stdin();
    for line_data in stdin.lock().lines() {
        if let Ok(line) = line_data {
            let ln = line.trim();

            if let Some(_) = create_map.get(ln) {
                // Green for create
                write!(stdout, "\x1b[32m{}\x1b[0m ", CIRCLE).unwrap();
                print_lscolor_path(&mut stdout, &ls_colors, &ln).unwrap();
                writeln!(stdout).unwrap();
            } else if let Some(_) = delete_map.get(ln) {
                // Red for removal
                write!(stdout, "\x1b[31m{}\x1b[0m ", CIRCLE).unwrap();
                print_lscolor_path(&mut stdout, &ls_colors, &ln).unwrap();
                writeln!(stdout).unwrap();
            } else if let (Some(_), Some(percent)) = (to_map.get(ln), percent_map.get(ln)) {
                // Red for renamed delete. yellow for percent
                write!(stdout, "\x1b[31m{}\x1b[0m ", LARROW).unwrap();
                print_lscolor_path(&mut stdout, &ls_colors, &ln).unwrap();
                write!(stdout, "\t\t\x1b[33m({})\x1b[0m", percent).unwrap();
                writeln!(stdout).unwrap();
            } else if let (Some(_), Some(percent)) = (from_map.get(ln), percent_map.get(ln)) {
                // Green for renamed create. yellow for percent
                write!(stdout, "\x1b[32m{}\x1b[0m ", RARROW).unwrap();
                print_lscolor_path(&mut stdout, &ls_colors, &ln).unwrap();
                write!(stdout, "\t\t\x1b[33m({})\x1b[0m", percent).unwrap();
                writeln!(stdout).unwrap();
            } else {
                // Blue for normal
                write!(stdout, "\x1b[34m{}\x1b[0m ", SQUARE).unwrap();
                print_lscolor_path(&mut stdout, &ls_colors, &ln).unwrap();
                writeln!(stdout).unwrap();
            }
        }
    }
}
