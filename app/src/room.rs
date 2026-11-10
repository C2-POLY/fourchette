//! The Gamer skin's room (#218): the board's City view as the gamer's room. One shelf per zone
//! (`city::zone`, at most `MAX_SHELVES`, three on each wall, the zones past the fifth sharing
//! "Other zones"), one game box per ticket on it, in the state its status gives (`city::Stage`),
//! a collector box for a feature with a pip per sub-issue, a little bug on a bug's box. A shelf
//! holds `PER_SHELF` boxes, the most active first, and counts the rest in a `+k`. Also where the
//! session's mess lands (`litter`): its own spot for each piece, until past 16 pieces the piles
//! grow instead. Pure layout in the room's tiles; `app/room.rs` draws it.

use crate::city::{self, Building, Stage};
use crate::github::Item;
use crate::xp::Piece;
use std::collections::BTreeMap;

/// The most shelves a room has; the zones past the fifth share the last one.
pub const MAX_SHELVES: usize = 6;
/// Shelves on each wall.
pub const PER_WALL: usize = 3;
/// Boxes a shelf shows: three boards of three, the last place left for the `+k`.
pub const PER_SHELF: usize = 8;
/// Boxes on one board.
pub const PER_BOARD: usize = 3;
/// The room's floor, in tiles.
pub const SIZE: f32 = 11.0;
/// Past this many pieces the mess piles up instead of spreading.
pub const SPREAD: usize = 16;
/// Pieces from which flies buzz over the mess.
pub const FLIES: usize = 15;

/// Which wall a shelf stands against: the right one runs along x at y = 0, the left one along y
/// at x = 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wall {
    Right,
    Left,
}

/// One ticket's game box.
#[derive(Clone, Debug, PartialEq)]
pub struct GameBox {
    pub id: String,
    pub number: Option<u64>,
    pub stage: Stage,
    pub building: Building,
    /// Sub-issues done and in all.
    pub subs: (u32, u32),
    /// Its place on the shelf: 0 the first, on the top board's left.
    pub slot: usize,
}

impl GameBox {
    /// Its board, 0 the lowest, and its place on it.
    pub fn board(&self) -> (usize, usize) {
        (2 - self.slot / PER_BOARD, self.slot % PER_BOARD)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shelf {
    pub name: String,
    pub wall: Wall,
    /// Where along its wall it starts and ends, in tiles.
    pub along: (f32, f32),
    pub boxes: Vec<GameBox>,
    /// The tickets past `PER_SHELF`.
    pub more: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Room {
    pub shelves: Vec<Shelf>,
}

/// How active a ticket is: the one being played first, the wishlist last.
fn activity(s: Stage) -> u8 {
    match s {
        Stage::Foundations => 0,
        Stage::Frame => 1,
        Stage::Testing => 2,
        Stage::Staked => 3,
        Stage::Built => 4,
        Stage::Lot => 5,
    }
}

/// Lays the tickets out: a shelf per zone, the biggest first (on the right wall), then by name.
pub fn layout(items: &[&Item]) -> Room {
    let mut by_zone: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
    for it in items {
        by_zone.entry(city::zone(it)).or_default().push(it);
    }
    let mut zones: Vec<(String, Vec<&Item>)> = by_zone.into_iter().collect();
    zones.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    if zones.len() > MAX_SHELVES {
        let rest: Vec<&Item> = zones.drain(MAX_SHELVES - 1..).flat_map(|z| z.1).collect();
        zones.push(("Other zones".into(), rest));
    }
    let shelves = zones
        .into_iter()
        .enumerate()
        .map(|(i, (name, mut tickets))| {
            tickets.sort_by_key(|it| (activity(Stage::of(&it.status)), it.number.unwrap_or(u64::MAX)));
            let boxes = tickets
                .iter()
                .take(PER_SHELF)
                .enumerate()
                .map(|(slot, it)| GameBox {
                    id: it.id.clone(),
                    number: it.number,
                    stage: Stage::of(&it.status),
                    building: Building::of(it),
                    subs: it.sub_progress,
                    slot,
                })
                .collect();
            let a0 = 0.5 + (i % PER_WALL) as f32 * 3.5;
            Shelf { name, wall: if i < PER_WALL { Wall::Right } else { Wall::Left }, along: (a0, a0 + 3.1), boxes, more: tickets.len().saturating_sub(PER_SHELF) }
        })
        .collect();
    Room { shelves }
}

/// Where a piece of mess lies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// Standing on the desk, at a tile.
    Desk(f32, f32),
    /// On the floor, at a tile.
    Floor(f32, f32),
    /// The n-th box of a pizza stack standing at a tile.
    Stack(f32, f32, usize),
    /// The n-th can of the pyramid on the desk.
    Pyramid(usize),
    /// The n-th bag of the heap in the corner.
    Heap(usize),
}

/// The mess as it lies in the room, and whether flies buzz over it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Litter {
    pub pieces: Vec<(Piece, Place)>,
    pub flies: bool,
}

/// The cans' spots: the first two on the desk, then on the floor.
pub const DESK_CANS: [(f32, f32); 2] = [(8.0, 4.95), (8.15, 4.6)];
pub const FLOOR_CANS: [(f32, f32); 8] = [(6.6, 9.8), (9.6, 8.2), (2.0, 9.4), (10.0, 9.9), (5.6, 10.6), (3.0, 10.6), (8.2, 10.7), (5.9, 10.3)];
pub const BAGS: [(f32, f32); 8] = [(2.4, 7.6), (4.6, 10.2), (9.6, 10.6), (1.4, 10.2), (7.0, 10.4), (1.2, 8.4), (10.4, 7.8), (3.9, 10.8)];
pub const SPILLS: [(f32, f32); 4] = [(4.4, 8.2), (7.6, 7.8), (2.6, 6.6), (9.2, 9.2)];
/// The two pizza stacks, the first of which grows into a tower once the room is crowded.
pub const STACKS: [(f32, f32); 2] = [(8.8, 8.6), (1.2, 5.6)];
/// Boxes in a stack before the next one starts, a tower's most, the pyramid's and the heap's.
pub const STACK: usize = 5;
pub const TOWER: usize = 14;
pub const PYRAMID: usize = 15;
pub const HEAP: usize = 12;

/// Where each piece of `mess` lands, in the order it was left: its own spot while the room has
/// room, the piles past `SPREAD` pieces (a can pyramid on the desk, a pizza tower, a heap of bags).
/// What doesn't fit on a full pile isn't drawn.
pub fn litter(mess: &[Piece]) -> Litter {
    let mut out = Litter { pieces: Vec::new(), flies: mess.len() >= FLIES };
    let (mut cans, mut floor_cans, mut pyramid, mut bags, mut heap, mut spills) = (0, 0, 0, 0, 0, 0);
    let mut stacks = [0usize; 2];
    for (i, piece) in mess.iter().enumerate() {
        let crowded = i >= SPREAD;
        let place = match piece {
            Piece::Can => {
                cans += 1;
                if !crowded && cans <= DESK_CANS.len() {
                    Some(Place::Desk(DESK_CANS[cans - 1].0, DESK_CANS[cans - 1].1))
                } else if !crowded && floor_cans < FLOOR_CANS.len() {
                    floor_cans += 1;
                    Some(Place::Floor(FLOOR_CANS[floor_cans - 1].0, FLOOR_CANS[floor_cans - 1].1))
                } else {
                    pyramid += 1;
                    (pyramid <= PYRAMID).then(|| Place::Pyramid(pyramid - 1))
                }
            }
            Piece::Pizza => {
                let s = if !crowded && stacks[0] >= STACK && stacks[1] < STACK { 1 } else { 0 };
                stacks[s] += 1;
                let cap = if s == 0 { TOWER } else { STACK };
                (stacks[s] <= cap).then(|| Place::Stack(STACKS[s].0, STACKS[s].1, stacks[s] - 1))
            }
            Piece::Bag => {
                if !crowded && bags < BAGS.len() {
                    bags += 1;
                    Some(Place::Floor(BAGS[bags - 1].0, BAGS[bags - 1].1))
                } else {
                    heap += 1;
                    (heap <= HEAP).then(|| Place::Heap(heap - 1))
                }
            }
            Piece::Spill => {
                spills += 1;
                let (x, y) = SPILLS[(spills - 1) % SPILLS.len()];
                Some(Place::Floor(x, y))
            }
        };
        if let Some(place) = place {
            out.pieces.push((*piece, place));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Kind, Label};

    fn ticket(n: u64, zone: &str, status: &str) -> Item {
        Item {
            id: format!("i{n}"),
            number: Some(n),
            status: status.into(),
            kind: Kind::Issue,
            issue_type: Some(("Task".into(), String::new())),
            labels: vec![Label { name: format!("zone:{zone}"), color: String::new() }],
            ..Default::default()
        }
    }

    fn of(items: &[Item]) -> Room {
        layout(&items.iter().collect::<Vec<_>>())
    }

    #[test]
    fn one_shelf_per_zone_three_on_each_wall() {
        let items: Vec<Item> = (0..5).map(|i| ticket(i, &format!("z{i}"), "Ready")).chain([ticket(9, "z3", "Done")]).collect();
        let r = of(&items);
        assert_eq!(r.shelves.len(), 5);
        assert_eq!(r.shelves[0].name, "z3", "the biggest zone first");
        assert_eq!(r.shelves.iter().filter(|s| s.wall == Wall::Right).count(), PER_WALL);
        assert_eq!(r.shelves[3].wall, Wall::Left);
        assert_eq!(r.shelves[1].along, (4.0, 7.1));
        assert!(r.shelves.iter().all(|s| s.along.1 < SIZE));
    }

    #[test]
    fn zones_past_the_fifth_share_the_last_shelf() {
        let items: Vec<Item> = (0..9).map(|i| ticket(i, &format!("z{i}"), "In progress")).collect();
        let r = of(&items);
        assert_eq!(r.shelves.len(), MAX_SHELVES);
        let last = r.shelves.last().unwrap();
        assert_eq!((last.name.as_str(), last.boxes.len()), ("Other zones", 4));
    }

    #[test]
    fn a_full_shelf_shows_the_most_active_first_and_counts_the_rest() {
        let statuses = ["Done", "Backlog", "Ready", "In progress", "To test", "In review", "Done", "Backlog", "Done", "Ready", "Done"];
        let items: Vec<Item> = statuses.iter().enumerate().map(|(i, s)| ticket(i as u64 + 1, "a", s)).collect();
        let r = of(&items);
        let s = &r.shelves[0];
        assert_eq!((s.boxes.len(), s.more), (PER_SHELF, 3));
        let stages: Vec<Stage> = s.boxes.iter().map(|b| b.stage).collect();
        assert_eq!(&stages[..5], [Stage::Foundations, Stage::Frame, Stage::Testing, Stage::Staked, Stage::Staked]);
        assert!(!stages.contains(&Stage::Lot), "the wishlist is what is left out");
        // The first on the top board, on the left.
        assert_eq!((s.boxes[0].board(), s.boxes[4].board(), s.boxes[7].board()), ((2, 0), (1, 1), (0, 1)));
        assert_eq!(s.boxes[0].number, Some(4));
    }

    #[test]
    fn each_piece_has_its_spot_until_the_piles_grow() {
        let l = litter(&[Piece::Can, Piece::Can, Piece::Can, Piece::Pizza, Piece::Bag, Piece::Spill]);
        assert_eq!(l.pieces[0].1, Place::Desk(8.0, 4.95));
        assert_eq!(l.pieces[2].1, Place::Floor(6.6, 9.8), "the third can on the floor");
        assert_eq!(l.pieces[3].1, Place::Stack(8.8, 8.6, 0));
        assert_eq!(l.pieces[5].1, Place::Floor(4.4, 8.2));
        assert!(!l.flies);
        // Past 16 pieces the cans go on the pyramid, the boxes on the tower, the bags on the heap.
        let mut mess = vec![Piece::Pizza; 7];
        mess.extend([Piece::Can; 9]);
        mess.extend([Piece::Can, Piece::Pizza, Piece::Bag]);
        let l = litter(&mess);
        assert_eq!(l.pieces[6].1, Place::Stack(1.2, 5.6, 1), "the second stack once the first has 5");
        assert_eq!(l.pieces[16].1, Place::Pyramid(0));
        assert_eq!(l.pieces[17].1, Place::Stack(8.8, 8.6, 5), "the tower");
        assert_eq!(l.pieces[18].1, Place::Heap(0));
        assert!(l.flies);
        // A full pile takes no more.
        assert_eq!(litter(&[Piece::Pizza; 40]).pieces.len(), TOWER + STACK);
    }
}
