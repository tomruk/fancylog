mod config;
mod field;
mod reader;
mod reader_builder;
mod reader_json;
mod reader_regex;
mod source;

use crate::config::{Config, Exclude, Format, Include};
use crate::reader_builder::ReaderBuilder;
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

    let follow = *matches.get_one::<bool>("follow").unwrap_or(&false);

    let mut futs = vec![];
    let mut readers = vec![];
    let mut reader_builder = ReaderBuilder::new(config)?;

    if let Some(sources) = matches.get_many::<String>("source") {
        let sources = sources.map(|source| source.to_string()).collect();
        readers = reader_builder.build(sources).await?;
    } else {
        readers = reader_builder.build(vec![]).await?;
    }

    if readers.len() == 0 {
        bail!("No files are given as argument and there is no input on stdin.");
    }

    for reader in readers {
        let source_type = reader.source_type();

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
