mod field;
mod reader;
mod reader_json;
mod source;

use crate::source::Stdin;
use anyhow::anyhow;
use clap::{arg, ArgAction, Command};
use futures::{
    channel::mpsc::{channel, Receiver},
    future::join_all,
    SinkExt, StreamExt,
};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use reader::Reader;
use reader_json::JsonReader;
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

    match (
        atty::is(atty::Stream::Stdout),
        atty::is(atty::Stream::Stderr),
        atty::is(atty::Stream::Stdin),
    ) {
        (true, true, false) => {
            println!("stdin will be used.");
            let stdin = io::stdin();
            let stdin = BufReader::new(Stdin::new(stdin));
            let source = Source::new(SourceType::Stdin, stdin);
            sources.push(source);
        }
        (stdout, stderr, stdin) => {
            println!("stdin will not be used. atty values are:\nstdout: {stdout}\nstderr: {stderr}\nstdin: {stdin}");
        }
    }

    for source in sources {
        let file_name: String = {
            match source.source_type() {
                SourceType::Stdin => "<stdin>".to_string(),
                SourceType::File(file_name) => file_name.clone(),
            }
        };
        let mut reader = JsonReader::new(source);
        let (mut watcher, mut rx) = new_async_watcher().map_err(|e| anyhow!(e))?;

        let fut = tokio::task::spawn(async move {
            // Add a path to be watched. All files and directories at that path and
            // below will be monitored for changes.
            let mut watch = || {
                watcher
                    .watch(Path::new(&file_name), RecursiveMode::NonRecursive)
                    .map_err(|e| anyhow!(e));
            };
            watch();

            /* loop {
            let res = rx.next().await;
            if let Some(res) = res {
                match res {
                    Ok(event) => {
                        println!("changed: {:?}", event); */
            loop {
                match reader.read_fields().await {
                    Ok(fields) => {
                        println!("{:?}", fields);
                    }
                    Err(e) => {
                        println!("Error: {e}");
                        //break;
                    }
                }
            }
            //watch();
            /* }
                        Err(e) => println!("watch error: {:?}", e),
                    }
                } else {
                    println!("res None");
                }
            }*/
        });
        futs.push(fut);
    }

    join_all(futs).await;
    Ok(())
}

fn new_async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
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
