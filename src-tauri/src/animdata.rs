//! Parses PMD SpriteCollab AnimData.xml into a typed table.

use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnimInfo {
    pub name: String,
    /// Sheet name after following CopyOf (`<source>-Anim.png`).
    pub source: String,
    pub frame_width: u32,
    pub frame_height: u32,
    /// Ticks per frame, 60 per second.
    pub durations: Vec<u32>,
    /// Direction rows; filled in by `AnimTable::validate`.
    pub rows: u32,
}

impl AnimInfo {
    pub fn total_secs(&self) -> f64 {
        self.durations.iter().sum::<u32>() as f64 / 60.0
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnimTable {
    pub anims: Vec<AnimInfo>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AnimError {
    #[error("AnimData.xml is not valid XML: {0}")]
    Xml(String),
    #[error("<Anim> without <Name>")]
    MissingName,
    #[error("{0}: bad CopyOf chain")]
    BadCopyOf(String),
    #[error("{0}: missing or invalid {1}")]
    MissingField(String, &'static str),
    #[error("{0}: no Durations")]
    NoDurations(String),
    #[error("missing sheet {0}-Anim.png")]
    MissingSheet(String),
    #[error("{name}: sheet is {width}x{height}, expected {columns} columns of {frame_width}x{frame_height}")]
    SheetSize { name: String, width: u32, height: u32, columns: u32, frame_width: u32, frame_height: u32 },
}

fn child_text(node: roxmltree::Node, tag: &str) -> Option<String> {
    node.children()
        .find(|c| c.has_tag_name(tag))
        .and_then(|c| c.text())
        .map(|t| t.trim().to_string())
}

pub fn parse(xml: &str) -> Result<AnimTable, AnimError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| AnimError::Xml(e.to_string()))?;
    let mut raw: Vec<(String, roxmltree::Node)> = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("Anim")) {
        let name = child_text(node, "Name").ok_or(AnimError::MissingName)?;
        raw.push((name, node));
    }
    let find = |name: &str| raw.iter().find(|(n, _)| n == name).map(|(_, node)| *node);

    let mut anims = Vec::new();
    for (name, _) in &raw {
        let mut source = name.clone();
        let mut seen = BTreeSet::from([name.clone()]);
        while let Some(target) = find(&source).and_then(|n| child_text(n, "CopyOf")) {
            if find(&target).is_none() || !seen.insert(target.clone()) {
                return Err(AnimError::BadCopyOf(name.clone()));
            }
            source = target;
        }
        let node = find(&source).expect("source exists");
        let number = |tag: &'static str| {
            child_text(node, tag)
                .and_then(|t| t.parse::<u32>().ok())
                .filter(|v| *v > 0)
                .ok_or_else(|| AnimError::MissingField(source.clone(), tag))
        };
        let frame_width = number("FrameWidth")?;
        let frame_height = number("FrameHeight")?;
        let durations: Vec<u32> = node
            .children()
            .find(|c| c.has_tag_name("Durations"))
            .map(|d| {
                d.children()
                    .filter(|c| c.has_tag_name("Duration"))
                    .filter_map(|c| c.text()?.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        if durations.is_empty() {
            return Err(AnimError::NoDurations(source));
        }
        anims.push(AnimInfo { name: name.clone(), source, frame_width, frame_height, durations, rows: 0 });
    }
    Ok(AnimTable { anims })
}

impl AnimTable {
    pub fn get(&self, name: &str) -> Option<&AnimInfo> {
        self.anims.iter().find(|a| a.name == name)
    }

    pub fn sources(&self) -> BTreeSet<String> {
        self.anims.iter().map(|a| a.source.clone()).collect()
    }

    /// Checks every sheet against its frame size and fills in `rows`.
    pub fn validate(&mut self, sheet_size: impl Fn(&str) -> Option<(u32, u32)>) -> Result<(), AnimError> {
        for a in &mut self.anims {
            let (width, height) = sheet_size(&a.source).ok_or_else(|| AnimError::MissingSheet(a.source.clone()))?;
            let columns = a.durations.len() as u32;
            if width != a.frame_width * columns || height == 0 || height % a.frame_height != 0 {
                return Err(AnimError::SheetSize {
                    name: a.source.clone(),
                    width,
                    height,
                    columns,
                    frame_width: a.frame_width,
                    frame_height: a.frame_height,
                });
            }
            a.rows = height / a.frame_height;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Structure and numbers from Wobbuffet (0202): a CopyOf entry, a single-row sheet, varied durations.
    pub const WOBBUFFET_XML: &str = r#"<?xml version="1.0" ?>
<AnimData><ShadowSize>1</ShadowSize><Anims>
  <Anim><Name>Walk</Name><Index>0</Index><FrameWidth>24</FrameWidth><FrameHeight>32</FrameHeight>
    <Durations><Duration>8</Duration><Duration>10</Duration><Duration>8</Duration><Duration>10</Duration></Durations></Anim>
  <Anim><Name>Attack</Name><Index>1</Index><FrameWidth>64</FrameWidth><FrameHeight>72</FrameHeight>
    <Durations><Duration>2</Duration><Duration>4</Duration><Duration>1</Duration><Duration>1</Duration><Duration>1</Duration>
    <Duration>2</Duration><Duration>2</Duration><Duration>2</Duration><Duration>2</Duration><Duration>2</Duration></Durations></Anim>
  <Anim><Name>Strike</Name><Index>2</Index><CopyOf>Attack</CopyOf></Anim>
  <Anim><Name>Sleep</Name><Index>5</Index><FrameWidth>24</FrameWidth><FrameHeight>40</FrameHeight>
    <Durations><Duration>30</Duration><Duration>35</Duration></Durations></Anim>
  <Anim><Name>Idle</Name><Index>7</Index><FrameWidth>24</FrameWidth><FrameHeight>40</FrameHeight>
    <Durations><Duration>40</Duration><Duration>4</Duration><Duration>4</Duration><Duration>4</Duration><Duration>4</Duration>
    <Duration>4</Duration><Duration>4</Duration><Duration>4</Duration><Duration>4</Duration></Durations></Anim>
</Anims></AnimData>"#;

    pub fn wobbuffet_sizes(source: &str) -> Option<(u32, u32)> {
        match source {
            "Walk" => Some((96, 256)),
            "Attack" => Some((640, 576)),
            "Sleep" => Some((48, 40)),
            "Idle" => Some((216, 320)),
            _ => None,
        }
    }

    #[test]
    fn parses_in_file_order_and_resolves_copy_of() {
        let t = parse(WOBBUFFET_XML).unwrap();
        let names: Vec<&str> = t.anims.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Walk", "Attack", "Strike", "Sleep", "Idle"]);
        let strike = t.get("Strike").unwrap();
        assert_eq!(strike.source, "Attack");
        assert_eq!((strike.frame_width, strike.frame_height), (64, 72));
        assert_eq!(strike.durations.len(), 10);
        assert_eq!(t.sources().into_iter().collect::<Vec<_>>(), ["Attack", "Idle", "Sleep", "Walk"]);
    }

    #[test]
    fn total_secs_uses_60_ticks_per_second() {
        let t = parse(WOBBUFFET_XML).unwrap();
        assert!((t.get("Idle").unwrap().total_secs() - 72.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn validate_derives_rows_from_sheet_height() {
        let mut t = parse(WOBBUFFET_XML).unwrap();
        t.validate(wobbuffet_sizes).unwrap();
        assert_eq!(t.get("Idle").unwrap().rows, 8);
        assert_eq!(t.get("Sleep").unwrap().rows, 1);
        assert_eq!(t.get("Strike").unwrap().rows, 8);
    }

    #[test]
    fn validate_rejects_wrong_sheet_width() {
        let mut t = parse(WOBBUFFET_XML).unwrap();
        let err = t
            .validate(|s| if s == "Walk" { Some((100, 256)) } else { wobbuffet_sizes(s) })
            .unwrap_err();
        assert!(matches!(err, AnimError::SheetSize { .. }), "{err}");
    }

    #[test]
    fn validate_rejects_missing_sheet() {
        let mut t = parse(WOBBUFFET_XML).unwrap();
        let err = t.validate(|s| if s == "Idle" { None } else { wobbuffet_sizes(s) }).unwrap_err();
        assert_eq!(err, AnimError::MissingSheet("Idle".into()));
    }

    #[test]
    fn rejects_copy_of_cycles_and_dangling_targets() {
        let cycle = r#"<AnimData><Anims>
            <Anim><Name>A</Name><CopyOf>B</CopyOf></Anim>
            <Anim><Name>B</Name><CopyOf>A</CopyOf></Anim></Anims></AnimData>"#;
        assert!(matches!(parse(cycle), Err(AnimError::BadCopyOf(_))));
        let dangling = r#"<AnimData><Anims><Anim><Name>A</Name><CopyOf>Nope</CopyOf></Anim></Anims></AnimData>"#;
        assert!(matches!(parse(dangling), Err(AnimError::BadCopyOf(_))));
    }

    #[test]
    fn rejects_invalid_xml_and_missing_fields() {
        assert!(matches!(parse("<AnimData><Anims>"), Err(AnimError::Xml(_))));
        let no_width = r#"<AnimData><Anims><Anim><Name>A</Name><FrameHeight>8</FrameHeight>
            <Durations><Duration>1</Duration></Durations></Anim></Anims></AnimData>"#;
        assert!(matches!(parse(no_width), Err(AnimError::MissingField(_, "FrameWidth"))));
        let no_durations = r#"<AnimData><Anims><Anim><Name>A</Name><FrameWidth>8</FrameWidth>
            <FrameHeight>8</FrameHeight></Anim></Anims></AnimData>"#;
        assert!(matches!(parse(no_durations), Err(AnimError::NoDurations(_))));
    }
}
