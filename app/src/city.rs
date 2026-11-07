//! The Project as a city (#190): one district per zone label, one plot per ticket, built as far
//! as the ticket's status goes. Pure layout on a grid of tiles; `app/city.rs` draws it in
//! isometric view.

use crate::github::{Item, Kind};
use std::collections::BTreeMap;

/// How far a ticket's building has gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Backlog, or a status the city doesn't know: bare ground.
    Lot,
    /// Ready: the plot is staked out.
    Staked,
    /// In progress: foundations under a crane.
    Foundations,
    /// In review: the building half up, still under its crane.
    Frame,
    /// To test: built, with a flag on the roof.
    Testing,
    /// Done: built.
    Built,
}

impl Stage {
    pub fn of(status: &str) -> Stage {
        let s = status.to_lowercase();
        if crate::github::is_done(&s) {
            Stage::Built
        } else if s.contains("test") {
            Stage::Testing
        } else if s.contains("review") {
            Stage::Frame
        } else if s.contains("progress") || s.contains("doing") || s.contains("cours") {
            Stage::Foundations
        } else if s.contains("ready") || s.contains("todo") || s.contains("to do") {
            Stage::Staked
        } else {
            Stage::Lot
        }
    }

    /// A crane stands over the plot.
    pub fn crane(self) -> bool {
        matches!(self, Stage::Foundations | Stage::Frame)
    }

    pub fn built(self) -> bool {
        matches!(self, Stage::Testing | Stage::Built)
    }
}

/// What a finished ticket becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Building {
    /// A Feature: a tower, taller with more sub-issues.
    Tower,
    /// A Task, a draft, a pull request: a brick building.
    House,
    /// A Bug: a low white building.
    Workshop,
}

impl Building {
    pub fn of(it: &Item) -> Building {
        match (&it.kind, it.issue_type.as_ref().map(|t| t.0.to_lowercase())) {
            (Kind::Issue, Some(t)) if t == "feature" => Building::Tower,
            (Kind::Issue, Some(t)) if t == "bug" => Building::Workshop,
            _ => Building::House,
        }
    }

    /// Floors once built: a tower gains one per sub-issue, up to nine.
    pub fn floors(self, sub_total: u32) -> u32 {
        match self {
            Building::Tower => (3 + sub_total).min(9),
            Building::House => 3,
            Building::Workshop => 2,
        }
    }
}

/// A ticket's tile.
#[derive(Clone, Debug, PartialEq)]
pub struct Plot {
    /// The project item's id.
    pub id: String,
    /// Tile coordinates: x to the east, y to the south.
    pub x: i32,
    pub y: i32,
    pub stage: Stage,
    pub building: Building,
    pub floors: u32,
    /// Index of its district in `City::districts`.
    pub district: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct District {
    pub name: String,
    /// Its block of tiles: top-left tile and size.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct City {
    pub districts: Vec<District>,
    /// In drawing order: from the back of the city to the front.
    pub plots: Vec<Plot>,
    /// Tiles the whole city covers, roads included: (columns, rows).
    pub size: (i32, i32),
    /// The tiles of the blocks no ticket takes: a tree grows on each.
    pub trees: Vec<(i32, i32)>,
    /// Parks where a row of blocks ends short of the widest one: top-left tile and size.
    pub parks: Vec<(i32, i32, i32, i32)>,
}

/// The district a ticket goes to: its first `zone:` label, else "Elsewhere".
pub fn zone(it: &Item) -> String {
    it.labels.iter().find_map(|l| l.name.strip_prefix("zone:").map(str::to_string)).unwrap_or_else(|| "Elsewhere".into())
}

/// Lays the tickets out: districts by size (the biggest first, then by name), each a near-square
/// block with the built plots at the back and the bare ones at the front, one tile of road
/// around every block, the blocks in rows of about the city's square root.
pub fn layout(items: &[&Item]) -> City {
    let mut by_zone: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
    for it in items {
        by_zone.entry(zone(it)).or_default().push(it);
    }
    let mut zones: Vec<(String, Vec<&Item>)> = by_zone.into_iter().collect();
    zones.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    let per_row = (zones.len() as f32).sqrt().ceil().max(1.0) as usize;
    let mut city = City::default();
    let (mut x, mut y, mut row_h, mut width) = (1, 1, 0, 0);
    // Each row of blocks: its top, its height and where its last block ends.
    let mut rows: Vec<(i32, i32, i32)> = Vec::new();
    for (n, (name, mut tickets)) in zones.into_iter().enumerate() {
        if n > 0 && n % per_row == 0 {
            rows.push((y, row_h, x));
            x = 1;
            y += row_h + 1;
            row_h = 0;
        }
        tickets.sort_by_key(|it| (std::cmp::Reverse(Stage::of(&it.status)), it.number.unwrap_or(u64::MAX)));
        let w = (tickets.len() as f32).sqrt().ceil().max(1.0) as i32;
        let h = (tickets.len() as i32 + w - 1) / w;
        let district = city.districts.len();
        for (i, it) in tickets.iter().enumerate() {
            let building = Building::of(it);
            city.plots.push(Plot {
                id: it.id.clone(),
                x: x + i as i32 % w,
                y: y + i as i32 / w,
                stage: Stage::of(&it.status),
                building,
                floors: building.floors(it.sub_progress.1),
                district,
            });
        }
        for i in tickets.len() as i32..w * h {
            city.trees.push((x + i % w, y + i / w));
        }
        city.districts.push(District { name, x, y, w, h });
        x += w + 1;
        width = width.max(x);
        row_h = row_h.max(h);
    }
    if row_h > 0 {
        rows.push((y, row_h, x));
    }
    city.size = (width, y + row_h + 1);
    // A park, with its trees, where a row ends short.
    for (py, ph, end) in rows {
        let pw = width - 1 - end;
        if pw > 0 {
            city.parks.push((end, py, pw, ph));
            for t in 0..pw * ph {
                city.trees.push((end + t % pw, py + t / pw));
            }
        }
    }
    // The painter's order of an isometric view: the farther a tile from the viewer, first.
    city.plots.sort_by_key(|p| (p.x + p.y, p.x));
    city
}

/// How many tickets are built, under construction and still to do.
pub fn counts(city: &City) -> (usize, usize, usize) {
    let built = city.plots.iter().filter(|p| p.stage.built()).count();
    let site = city.plots.iter().filter(|p| p.stage.crane()).count();
    (built, site, city.plots.len() - built - site)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::Label;

    fn item(n: u64, status: &str, zone: Option<&str>, ty: Option<&str>, subs: u32) -> Item {
        let mut it = crate::github::sample().items.into_iter().next().unwrap();
        it.id = format!("I{n}");
        it.number = Some(n);
        it.status = status.into();
        it.kind = Kind::Issue;
        it.labels = zone.map(|z| vec![Label { name: format!("zone:{z}"), color: "000000".into() }]).unwrap_or_default();
        it.issue_type = ty.map(|t| (t.into(), "BLUE".into()));
        it.sub_progress = (0, subs);
        it
    }

    #[test]
    fn statuses_become_stages() {
        assert_eq!(Stage::of("Backlog"), Stage::Lot);
        assert_eq!(Stage::of("Ready"), Stage::Staked);
        assert_eq!(Stage::of("In progress"), Stage::Foundations);
        assert_eq!(Stage::of("In review"), Stage::Frame);
        assert_eq!(Stage::of("To test"), Stage::Testing);
        assert_eq!(Stage::of("Done"), Stage::Built);
        assert!(Stage::of("In review").crane() && !Stage::of("Done").crane());
    }

    #[test]
    fn types_become_buildings() {
        assert_eq!(Building::of(&item(1, "Done", None, Some("Feature"), 3)), Building::Tower);
        assert_eq!(Building::of(&item(2, "Done", None, Some("Bug"), 0)), Building::Workshop);
        assert_eq!(Building::of(&item(3, "Done", None, Some("Task"), 0)), Building::House);
        assert_eq!(Building::Tower.floors(3), 6);
        assert_eq!(Building::Tower.floors(40), 9);
    }

    #[test]
    fn districts_hold_their_zone_without_overlap() {
        let items: Vec<Item> = vec![
            item(1, "Done", Some("graphe"), Some("Feature"), 0),
            item(2, "Backlog", Some("graphe"), None, 0),
            item(3, "In progress", Some("graphe"), None, 0),
            item(4, "Done", Some("github"), None, 0),
            item(5, "Ready", None, None, 0),
        ];
        let refs: Vec<&Item> = items.iter().collect();
        let city = layout(&refs);
        assert_eq!(city.districts.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["graphe", "Elsewhere", "github"]);
        assert_eq!(city.plots.len(), 5);
        // Every plot inside its district's block, and no two on one tile.
        for p in &city.plots {
            let d = &city.districts[p.district];
            assert!(p.x >= d.x && p.x < d.x + d.w && p.y >= d.y && p.y < d.y + d.h, "{p:?} outside {d:?}");
        }
        let mut tiles: Vec<(i32, i32)> = city.plots.iter().map(|p| (p.x, p.y)).collect();
        tiles.sort();
        tiles.dedup();
        assert_eq!(tiles.len(), 5);
        // Built first within a district: the Done ticket gets the block's first tile.
        let done = city.plots.iter().find(|p| p.id == "I1").unwrap();
        assert_eq!((done.x, done.y), (city.districts[0].x, city.districts[0].y));
        assert_eq!(counts(&city), (2, 1, 2));
        // The block of three keeps its fourth tile for a tree.
        let d = &city.districts[0];
        let in_block: Vec<_> = city.trees.iter().filter(|(x, y)| *x >= d.x && *x < d.x + d.w && *y >= d.y && *y < d.y + d.h).collect();
        assert_eq!(in_block, vec![&(d.x + 1, d.y + 1)]);
        // Drawn back to front.
        assert!(city.plots.windows(2).all(|w| w[0].x + w[0].y <= w[1].x + w[1].y));
    }

    #[test]
    fn a_short_row_ends_in_a_park() {
        // Three districts: two on the first row, one on the second, narrower than the first.
        let items: Vec<Item> = vec![item(1, "Done", Some("a"), None, 0), item(2, "Done", Some("a"), None, 0), item(3, "Done", Some("b"), None, 0), item(4, "Done", Some("c"), None, 0)];
        let refs: Vec<&Item> = items.iter().collect();
        let city = layout(&refs);
        assert_eq!(city.parks.len(), 1);
        let (px, py, pw, ph) = city.parks[0];
        assert!(pw > 0 && ph > 0);
        // The park is road-free land past the last block of its row, inside the city.
        assert!(city.districts.iter().all(|d| d.y != py || d.x + d.w < px));
        assert!(px + pw < city.size.0);
    }

    #[test]
    fn an_empty_project_is_an_empty_city() {
        let city = layout(&[]);
        assert!(city.plots.is_empty() && city.districts.is_empty());
    }
}
