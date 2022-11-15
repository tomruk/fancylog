use crate::{
    config::{Config, Exclude, Format, Include},
    reader::Reader,
    reader_json::JsonReader,
    reader_regex::RegexReader,
    source::{Source, SourceType, Stdin},
};
use anyhow::{anyhow, bail};
use regex::Regex;
use std::collections::HashMap;
use tokio::{
    fs::File,
    io::{self, BufReader},
};

pub struct ReaderBuilder {
    config: Config,
    path_matches: HashMap<String, Regex>,
}

impl ReaderBuilder {
    pub fn new(config: Config) -> anyhow::Result<Self> {
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

        Ok(Self {
            config: config,
            path_matches: path_matches,
        })
    }

    pub async fn build(
        &mut self,
        sources: Vec<String>,
    ) -> anyhow::Result<Vec<Box<dyn Reader + Send>>> {
        let mut readers = Vec::with_capacity(sources.len());
        let mut stdin_used = false;

        for source in sources {
            let (reader, _stdin_used) = self.build_one(source).await?;
            if stdin_used == false {
                stdin_used = _stdin_used;
            }
            readers.push(reader);
        }

        println!("stdin_used: {stdin_used}");

        // If there's an stdin input but there's no stdin found.
        if !stdin_used && atty::isnt(atty::Stream::Stdin) {
            let stdin = io::stdin();
            let stdin = BufReader::new(Stdin::new(stdin));
            let source = Source::new(SourceType::Stdin, stdin);

            let format = match &self.config.default_format {
                Some(format) => self.find_format(&format.clone())?,
                None => bail!("stdin is used but there's no format defined for it"),
            };

            readers.push(self.new_reader(source, format)?);
        }

        Ok(readers)
    }

    async fn build_one(
        &mut self,
        source: String,
    ) -> anyhow::Result<(Box<dyn Reader + Send>, bool)> {
        let mut stdin_used = false;

        // Format: <format>:<file_path>
        // e.g. nginx:data/log/access.log
        let (format_name, file_path) = if let Some(semicolon_loc) = source.find(":") {
            let format_name = source[..semicolon_loc].to_string();
            let file_path = &source[semicolon_loc + 1..];
            (format_name, file_path)
        } else {
            let file_path = source.as_str();
            let format_name = self.format_name_from_file_path(file_path)?;
            (format_name, file_path)
        };

        let format = self.find_format(&format_name)?;

        let source = if file_path == "stdin" {
            if atty::is(atty::Stream::Stdin) {
                bail!("stdin was defined but it is not in use");
            }
            stdin_used = true;

            let stdin = io::stdin();
            let stdin = BufReader::new(Stdin::new(stdin));
            Source::new(SourceType::Stdin, stdin)
        } else {
            println!("{file_path}");
            let file = File::open(file_path).await?;
            let source = BufReader::new(file);
            Source::new(SourceType::File(file_path.to_string()), source)
        };

        Ok((self.new_reader(source, format)?, stdin_used))
    }

    fn new_reader(&self, source: Source, format: Format) -> anyhow::Result<Box<dyn Reader + Send>> {
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
                Box::new(RegexReader::new(source, re))
            }
        };
        Ok(reader)
    }

    fn format_name_from_file_path(&self, file_path: &str) -> anyhow::Result<String> {
        for (format_name, re) in &self.path_matches {
            if re.is_match(file_path) {
                return Ok(format_name.clone());
            }
        }

        match &self.config.default_format {
            Some(ref format) => return Ok(format.clone()),
            None => {
                bail!(
                    "no path matches found for {}. there is no default_format set either. exiting.",
                    file_path
                );
            }
        }
    }

    fn find_format(&mut self, format_name: &str) -> anyhow::Result<Format> {
        // Used `remove` to take ownership.
        match self.config.formats.remove(format_name) {
            Some(format) => Ok(format),
            None => bail!("there's no format with name: {}", format_name),
        }
    }
}
