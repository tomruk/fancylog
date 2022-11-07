mod field;
mod reader;
mod reader_json;
mod reader_regex;
mod source;

use crate::source::Stdin;
use anyhow::anyhow;
use clap::{arg, ArgAction, Command};
use futures::{
    channel::mpsc::{channel, Receiver},
    future::join_all,
    SinkExt, StreamExt,
};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Result, Watcher};
use reader::{ReadError, Reader};
use reader_json::JsonReader;
use reader_regex::RegexReader;
use regex::Regex;
use source::{Source, SourceType};
use std::{path::Path, time::Duration};
use tokio::{
    fs::File,
    io::{self, BufReader},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = Command::new("f")
        .arg(arg!([source] "Files to operate on").action(ArgAction::Append))
        .get_matches();

    let mut futs = vec![];
    let mut sources = vec![];

    if let Some(files) = matches.get_many::<String>("source") {
        for file_name in files {
            let file = File::open(&file_name).await?;
            let source = BufReader::new(file);
            let source = Source::new(SourceType::File(file_name.clone()), source);
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

    for source in sources {
        let source_type = source.source_type();
        let mut reader = JsonReader::new(source);
        //let mut reader = RegexReader::new(Regex::new(r"").unwrap(), source);

        let fut = match source_type {
            SourceType::Stdin => tokio::task::spawn(async move {
                read_stdin(reader).await;
            }),
            SourceType::File(file_name) => {
                let (mut watcher, mut rx) = new_async_watcher().map_err(|e| anyhow!(e))?;

                watcher
                    .watch(Path::new(&file_name), RecursiveMode::NonRecursive)
                    .map_err(|e| anyhow!(e));

                tokio::task::spawn(async move {
                    read_file(file_name, reader, watcher, rx).await;
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
        Config::default().with_poll_interval(Duration::from_millis(2000)),
    )?;

    Ok((watcher, rx))
}

async fn read_stdin<R: Reader>(mut reader: R) {
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

async fn read_file<R, W>(
    file_name: String,
    mut reader: R,
    mut watcher: W,
    mut rx: Receiver<notify::Result<Event>>,
) where
    R: Reader,
    W: Watcher,
{
    println!("Reading {file_name}");

    loop {
        loop {
            match reader.read_fields().await {
                Ok(fields) => {
                    println!("{:?}", fields);
                }
                Err(e) => {
                    if e == ReadError::Eof {
                        println!("{file_name} EOF");
                        break;
                    }
                    println!("Error: {file_name}: {e}");
                }
            }
        }

        let res = rx.next().await;
        if let Some(res) = res {
            match res {
                Ok(event) => {
                    println!("changed: {:?}", event);
                    continue;
                }
                Err(e) => println!("watch error: {file_name} {:?}", e),
            }
        } else {
            println!("{file_name} res is None");
        }
    }
}
