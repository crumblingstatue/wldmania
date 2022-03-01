//! Requirements file parsing

use crate::world::ChestType;
use crate::ItemIdMap;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct Requirement<Tracker: Default> {
    pub id: u16,
    pub n_stacks: u16,
    pub min_per_stack: u16,
    pub max_per_stack: u16,
    pub only_in: Vec<ChestType>,
    /// Embedded tracker that you can use to associate tracking data with this requirement.
    /// For example, you could track how many times the item has been found.
    /// You can use `()` if you don't need tracking.
    pub tracker: Tracker,
    pub prefix_id: u8,
}

enum Segment {
    NStacks(u16),
    StackRange(u16, u16),
    OnlyIn(Vec<ChestType>),
}

fn parse_only_in(seg: &str) -> Result<Segment, Box<dyn Error>> {
    let mut only_in = Vec::new();
    let names = seg.split('/');
    for name in names {
        let name = name.trim();
        match ChestType::from_str(name) {
            Some(type_) => only_in.push(type_),
            None => return Err(format!("Invalid chest type: {}", name).into()),
        }
    }
    Ok(Segment::OnlyIn(only_in))
}

fn parse_n_stacks_or_stack_range(seg: &str) -> Result<Segment, Box<dyn Error>> {
    let seg = seg.trim();
    Ok(match seg.find('-') {
        None => Segment::NStacks(seg.parse()?),
        Some(hyphen) => Segment::StackRange(
            seg[..hyphen].trim().parse()?,
            seg[hyphen + 1..].trim().parse()?,
        ),
    })
}

fn parse_segment(seg: &str) -> Result<Segment, Box<dyn Error>> {
    if seg.starts_with(|c: char| c.is_alphabetic()) {
        parse_only_in(seg)
    } else {
        parse_n_stacks_or_stack_range(seg)
    }
}

impl<Tracker: Default> Requirement<Tracker> {
    fn parse(line: &str, id_map: &ItemIdMap) -> Result<Self, Box<dyn Error>> {
        let prefix_id;
        let from_name = if line.starts_with('*') {
            let first_space = line.find(' ').ok_or("Expected space after *Prefix")?;
            let prefix = &line[1..first_space];
            prefix_id = crate::prefix_names::id_by_name(prefix)
                .ok_or_else(|| format!("Invalid prefix: {}", prefix))?;
            &line[first_space + 1..]
        } else {
            prefix_id = 0;
            line
        };
        let mut n_stacks = None;
        let mut stack_range = None;
        let mut only_in = None;
        let end_of_name;
        if let Some(colon) = from_name.find(':') {
            let segments = from_name[colon + 1..].split(',');
            for seg in segments {
                let seg = seg.trim();
                if seg.is_empty() {
                    continue;
                }
                match parse_segment(seg)? {
                    Segment::NStacks(n) => match n_stacks {
                        None => n_stacks = Some(n),
                        Some(_) => return Err("Duplicate n-stacks segment".into()),
                    },
                    Segment::StackRange(min, max) => match stack_range {
                        None => stack_range = Some((min, max)),
                        Some(_) => return Err("Duplicate stack range segment".into()),
                    },
                    Segment::OnlyIn(vec) => match only_in {
                        None => only_in = Some(vec),
                        Some(_) => return Err("Duplicate only-in segment".into()),
                    },
                }
            }
            end_of_name = colon;
        } else {
            end_of_name = from_name.len();
        }
        let (min, max) = stack_range.unwrap_or((1, 1));
        let only_in = only_in.unwrap_or_default();
        Ok(Requirement {
            id: id_map
                .id_by_name(&from_name[..end_of_name])
                .ok_or_else(|| {
                    format!("No matching id for item '{}'", &from_name[..end_of_name])
                })?,
            n_stacks: n_stacks.unwrap_or(1),
            min_per_stack: min,
            max_per_stack: max,
            only_in,
            tracker: Default::default(),
            prefix_id,
        })
    }
}

#[test]
fn test_parse() {
    let item_ids = crate::item_ids();
    assert_eq!(
        Requirement::parse(
            "Sandstorm in a bottle: gold/locked shadow, 2, 3-7",
            &item_ids
        )
        .unwrap(),
        Requirement {
            id: 857,
            n_stacks: 2,
            min_per_stack: 3,
            max_per_stack: 7,
            only_in: vec![ChestType::Gold, ChestType::LockedShadow],
            tracker: (),
            prefix_id: 0,
        }
    )
}

#[test]
fn test_parse_no_extra() {
    let item_ids = crate::item_ids();
    assert_eq!(
        Requirement::parse("Sandstorm in a bottle", &item_ids).unwrap(),
        Requirement {
            id: 857,
            n_stacks: 1,
            min_per_stack: 1,
            max_per_stack: 1,
            only_in: vec![],
            tracker: (),
            prefix_id: 0,
        }
    )
}

pub fn from_path<Tracker: Default>(
    path: &Path,
    id_map: &ItemIdMap,
) -> Result<Vec<Requirement<Tracker>>, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    from_str(&buf, id_map)
}

pub fn from_str<Tracker: Default>(
    txt: &str,
    id_map: &ItemIdMap,
) -> Result<Vec<Requirement<Tracker>>, Box<dyn Error>> {
    let mut reqs = Vec::new();
    for (n, line) in txt.lines().enumerate() {
        let line = line.trim();
        if !(line.is_empty() || line.starts_with('#')) {
            reqs.push(
                Requirement::parse(line, id_map).map_err(|e| format!("Line {}: {}", n + 1, e))?,
            );
        }
    }
    Ok(reqs)
}
