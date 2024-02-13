#[macro_use]
extern crate serde_derive;

use anyhow::Result;
use clap::Parser;
use itertools::Itertools;
use std::collections::HashMap;
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
enum ProgramError {
    #[error("yabai executable failed")]
    YabayExecutionError(#[from] std::io::Error),
    #[error("parsing yabai output failed")]
    YabaiJsonParseError(#[from] serde_json::Error),
    #[error("illegal yabai state")]
    //YabaiConfigError(#[from] itertools::ExactlyOneError<_>),
    YabaiConfigError,
}

enum YabaiSpace {
    Next,
    Previous,
    Space(u32),
}

#[derive(Parser, Default, Debug)]
#[clap(
    author = "Dan Shechter",
    name = "yabai-cycle-spaces",
    version,
    about = "A command line to switch spaces in yabai"
)]
#[clap(group = clap::ArgGroup::new("cycle-group").multiple(false))]
struct Arguments {
    #[clap(short, long, group = "cycle-group")]
    next: bool,
    #[clap(short, long, group = "cycle-group")]
    prev: bool,
    #[clap(long, group = "cycle-group")]
    cycle_to: Option<u32>,
}

#[derive(Debug)]
struct YabaiSpaceConfig {
    display_space_map: HashMap<u32, Vec<u32>>,
    display_visible_map: HashMap<u32, u32>,
    focused_display: u32,
}

// [{
//    "id":1,
//    "uuid":"",
//    "index":1,
//    "label":"",
//    "type":"bsp",
//    "display":1,
//    "windows":[259, 93],
//    "first-window":93,
//    "last-window":93,
//    "has-focus":true,
//    "is-visible":true,
//    "is-native-fullscreen":false
//}]

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct YabaiSpaceConfigJson {
    id: u32,
    //uuid: String,
    //index: i32,
    //label: String,
    //#[serde(rename(serialize = "type", deserialize = "type"))]
    //space_type: String,
    display: u32,
    //windows: Vec<u32>,
    //first_window: u32,
    //last_window: u32,
    has_focus: bool,
    is_visible: bool,
    //is_native_fullscreen: bool,
}

fn yabai_query_spaces() -> Result<YabaiSpaceConfig, ProgramError> {
    let output = Command::new("yabai")
        .arg("-m")
        .arg("query")
        .arg("--spaces")
        .output()?;

    let output_str = String::from_utf8_lossy(&output.stdout).to_string();
    let output_str = output_str.as_str();

    let all_spaces: Vec<YabaiSpaceConfigJson> = serde_json::from_str(output_str)?;

    let mut config = YabaiSpaceConfig {
        display_space_map: HashMap::new(),
        display_visible_map: HashMap::new(),
        focused_display: all_spaces
            .iter()
            .filter(|s| s.has_focus)
            .exactly_one()
            .map_err(|_| ProgramError::YabaiConfigError)?
            .display,
    };

    for (display, spaces) in &all_spaces.into_iter().group_by(|s| s.display) {
        let spaces: Vec<YabaiSpaceConfigJson> = spaces.collect();
        let display_space_map = spaces.iter().map(|s| s.id).collect_vec();

        let focused_space = spaces
            .iter()
            .filter(|s| s.is_visible)
            .exactly_one()
            .map_err(|_| ProgramError::YabaiConfigError)?;

        config.display_visible_map.insert(display, focused_space.id);
        config.display_space_map.insert(display, display_space_map);
    }

    Ok(config)
}

fn yabai_focus_space(display: u32, space_idx: u32) -> Result<(), ProgramError> {
    let output = Command::new("yabai")
        .arg("-m")
        .arg("space")
        .arg("--focus")
        .arg((space_idx + 1).to_string())
        .output()?;

    Ok(())
}

fn yabai_move_space(config: &YabaiSpaceConfig, cmd: YabaiSpace) -> Result<(), ProgramError> {
    let focused_space = config
        .display_visible_map
        .get(&config.focused_display)
        .ok_or(ProgramError::YabaiConfigError)?;
    let selected_display_spaces = config
        .display_space_map
        .get(&config.focused_display)
        .ok_or(ProgramError::YabaiConfigError)?;

    let space_index = selected_display_spaces
        .iter()
        .enumerate()
        .filter(|(_, s)| *s == focused_space)
        .map(|(i, _)| i)
        .exactly_one()
        .map_err(|_| ProgramError::YabaiConfigError)?;

    let new_space = match cmd {
        YabaiSpace::Next => space_index + 1,
        YabaiSpace::Previous => space_index - 1,
        YabaiSpace::Space(s) => s as usize,
    };

    for (display, spaces) in config.display_space_map.iter() {
        let previous_spaces: usize = (0u32..*display - 1)
            .into_iter()
            .filter_map(|d| config.display_space_map.get(&(d + 1)))
            .map(|sv| sv.len())
            .sum();
        let new_space = previous_spaces as u32 + (new_space % spaces.len()) as u32;
        println!("selected space : {}", new_space);
        yabai_focus_space(*display, new_space)?;
    }

    Ok(())
}

fn main() -> Result<(), ProgramError> {
    let args = Arguments::parse();

    let ys = yabai_query_spaces()?;

    if args.next {
        yabai_move_space(&ys, YabaiSpace::Next)?;
    } else if args.prev {
        yabai_move_space(&ys, YabaiSpace::Previous)?;
    } else if let Some(cycle_to) = args.cycle_to {
        println!("cycle to {}", cycle_to);
    } else {
    }

    Ok(())
}
