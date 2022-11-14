mod config;
mod field;
mod reader;
mod reader_json;
mod reader_regex;
mod source;

use crate::config::{Config, Exclude, Format, Include};
use crate::source::Stdin;
use anyhow::{anyhow, bail};
use clap::{arg, crate_version, ArgAction, Command};
use futures::{
    channel::mpsc::{channel, Receiver},
    future::join_all,
    SinkExt, StreamExt,
};
use notify::{Event, RecommendedWatcher, RecursiveMode, Result, Watcher};
use reader::{ReadError, Reader};
use reader_json::JsonReader;
use reader_regex::RegexReader;
use regex::Regex;
use source::{Source, SourceType};
use std::{collections::HashMap, path::Path, process::exit, time::Duration};
use tokio::{
    fs::File,
    io::{self, BufReader},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = Command::new("f")
        .arg(arg!([source] "Files to operate on").action(ArgAction::Append))
        .arg(arg!(-f --follow "Print logs as they are appended. Works only on files. Usage is redundant with stdin input.").action(ArgAction::SetTrue))
        .version(crate_version!())
        .get_matches();

    let config = ::config::Config::builder()
        .add_source(::config::File::with_name("./f.yml"))
        .build()?;

    let mut config: Config = config.try_deserialize()?;
    let format = config.formats.get("custom_json").unwrap();
    println!("format: {:?}", format);

    let mut path_matches = HashMap::new();
    for (format_name, match_regex) in &config.path_matches {
        let re = Regex::new(match_regex).map_err(|e| {
            anyhow!(
                "invalid regex: `{}` because: {}",
                match_regex,
                e.to_string()
            )
        })?;
        path_matches.insert(format_name.clone(), re);
    }

    let follow = *matches.get_one::<bool>("follow").unwrap_or(&false);

    let mut futs = vec![];
    let mut sources = vec![];

    if let Some(files) = matches.get_many::<String>("source") {
        for file_path in files {
            let file = File::open(&file_path).await?;
            let source = BufReader::new(file);
            let source = Source::new(SourceType::File(file_path.clone()), source);
            sources.push(source);
        }
    }

    if atty::isnt(atty::Stream::Stdin) {
        println!("stdin will be used.");
        let stdin = io::stdin();
        let stdin = BufReader::new(Stdin::new(stdin));
        let source = Source::new(SourceType::Stdin, stdin);
        sources.push(source);
    } else {
        println!("stdin will not be used.");
    }

    if sources.len() == 0 {
        bail!("No files are given as argument and there is no input on stdin.");
    }

    for source in sources {
        let source_type = source.source_type();
        let mut matched_format_name = String::new();

        match source_type {
            SourceType::File(ref file_path) => {
                for (format_name, re) in &path_matches {
                    if re.is_match(file_path) {
                        matched_format_name = format_name.to_string();
                        break;
                    }
                }
                match config.default_format {
                    Some(ref format) => matched_format_name = format.clone(),
                    None => {
                        bail!("no path matches found for {}. there is no default_format set either. exiting.", file_path);
                    }
                }
            }
            SourceType::Stdin => {} // TODO: What if stdin is used?
        };

        let matched_format_name = matched_format_name.trim();
        // Used `remove` to take ownership.
        let format = match config.formats.remove(matched_format_name) {
            Some(format) => format,
            None => bail!("there's no format with name: {}", matched_format_name),
        };

        let reader: Box<dyn Reader + Send> = match format {
            Format::JsonFormat { exclude, include } => {
                let exclude = exclude.unwrap_or(Exclude::ExcludeMany(Vec::new()));
                let include = include.unwrap_or(Include::IncludeMany(Vec::new()));
                Box::new(JsonReader::new(source, exclude, include))
            }
            Format::RegexFormat { format } => {
                let re = Regex::new(&format).map_err(|e| {
                    anyhow!("regex failed for `{}` because {}", format, e.to_string())
                })?;
                Box::new(RegexReader::new(re, source))
            }
        };

        let fut = match source_type {
            SourceType::Stdin => tokio::task::spawn(async move {
                read_stdin(reader).await;
            }),
            SourceType::File(file_path) => {
                let (mut watcher, mut rx) = new_async_watcher().map_err(|e| anyhow!(e))?;

                watcher
                    .watch(Path::new(&file_path), RecursiveMode::NonRecursive)
                    .map_err(|e| anyhow!(e));

                tokio::task::spawn(async move {
                    read_file(file_path, follow, reader, watcher, rx).await;
                })
            }
        };

        futs.push(fut);
    }

    join_all(futs).await;
    Ok(())
}

fn new_async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    let watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        // TODO: Is 2 seconds a good idea?
        notify::Config::default().with_poll_interval(Duration::from_millis(2000)),
    )?;

    Ok((watcher, rx))
}

async fn read_stdin(mut reader: Box<dyn Reader + Send>) {
    println!("Read stdin");
    loop {
        match reader.read_fields().await {
            Ok(fields) => {
                println!("{:?}", fields);
            }
            Err(e) => {
                if e == ReadError::Eof {
                    println!("stdin EOF");
                    break;
                }
                println!("Error: stdin: {e}");
            }
        }
    }
}

async fn read_file(
    file_path: String,
    follow: bool,
    mut reader: Box<dyn Reader + Send>,
    mut watcher: impl Watcher,
    mut rx: Receiver<notify::Result<Event>>,
) {
    println!("Reading {file_path}");

    loop {
        loop {
            match reader.read_fields().await {
                Ok(fields) => {
                    println!("{:?}", fields);
                }
                Err(e) => {
                    if e == ReadError::Eof {
                        println!("{file_path} EOF");
                        break;
                    }
                    println!("Error: {file_path}: {e}");
                }
            }
        }

        if !follow {
            return;
        }

        let res = rx.next().await;
        if let Some(res) = res {
            match res {
                Ok(event) => {
                    println!("changed: {:?}", event);
                    continue;
                }
                Err(e) => println!("watch error: {file_path} {:?}", e),
            }
        } else {
            println!("{file_path} res is None");
        }
    }
}
