//! The orders grid of the panel and the two buttons above the minimap, as `Game_Dll.dll` builds them
//! (`0x10080fa0`) from what the selected units can do. See `docs/formats.md`, "Orders grid".

/// Capability bits of a unit (its vtable `+0x10`), ORed over the selection into `[0x103880d4]`.
pub mod can {
    pub const HOLD_POSITION: u32 = 1;
    pub const HOLD_FIRE: u32 = 2;
    pub const HALT: u32 = 4;
    pub const ADVANCE: u32 = 8;
    pub const ATTACK: u32 = 0x10;
    pub const ASSAULT: u32 = 0x20;
    pub const TURN: u32 = 0x1000;
    /// Added for any selection in single player (`0x10083820`).
    pub const SINGLE_PLAYER: u32 = 0x40_0000;
}

/// A tank's capabilities (`0x10014940`, `0x10010510`): it can always hold, hold fire and halt; when
/// not immobilised (`+0xb3`) also advance and turn; with the rounds for one of its weapons
/// (`0x10016fa0`) attack, and when mobile assault. The two checks of its cell for unloading
/// (`0x400`, `0x100`) are left out: no passengers yet.
pub fn tank(immobilised: bool, armed: bool) -> u32 {
    let mut c = can::HOLD_POSITION | can::HOLD_FIRE | can::HALT;
    if !immobilised {
        c |= can::ADVANCE | can::TURN;
    }
    if armed {
        c |= can::ATTACK;
        if !immobilised {
            c |= can::ASSAULT;
        }
    }
    c
}

/// The selection's capabilities: the OR of its units' (`0x10083820`), plus [`can::SINGLE_PLAYER`]
/// when there is any.
pub fn selection(units: impl IntoIterator<Item = u32>) -> u32 {
    let c = units.into_iter().fold(0, |a, b| a | b);
    if c != 0 { c | can::SINGLE_PLAYER } else { 0 }
}

/// A button of the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Button {
    /// Command id (`0x13` AVANCER...): the panel's notification (`0x10080150`).
    pub command: u8,
    /// `intrpic.dat` frame of its first state; the next frame is the second (order waiting for a
    /// target).
    pub frame: usize,
    /// String of `intrtext.dat` for its tooltip.
    pub text: usize,
}

pub const HALT: u8 = 0xb;
pub const HOLD_POSITION: u8 = 0xd;
pub const HOLD_FIRE: u8 = 0xe;
pub const ADVANCE: u8 = 0x13;

/// First layout record of the grid (`intrpic.dat` frame 0), row by row; the keys `key_00`-`key_22`
/// press the same places.
pub const GRID_RECORD: usize = 0x20;
/// Layout records of TENIR LA POSITION and SUSPENDRE LE FEU.
pub const HOLD_RECORDS: [usize; 2] = [7, 8];
/// Their pictures: three states each from these frames (all units, some, none).
pub const HOLD_FRAMES: [usize; 2] = [29, 32];

fn button(command: u8, frame: usize, tip: usize) -> Option<Button> {
    Some(Button { command, frame, text: tip + 1 })
}

/// The 9 places of the grid for capabilities `c`, as `0x10080fa0` fills them (places `0x25`-`0x28`
/// go through `0x10081a00` / `0x10081b00`, not read: their frame is the argument that looks like one).
pub fn grid(c: u32) -> [Option<Button>; 9] {
    let has = |bit: u32| c & bit != 0;
    let first = |options: &[(u32, u8, usize, usize)]| {
        options.iter().find(|o| has(o.0)).and_then(|&(_, command, frame, tip)| button(command, frame, tip))
    };
    [
        first(&[(8, 0x13, 51, 10)]),
        first(&[(0x10, 0x14, 53, 0xb), (0x800, 0x1c, 73, 0x10)]),
        first(&[(0x20, 0x15, 55, 0xc)]),
        first(&[(0x1000, 0x1d, 81, 0x11), (0x4000, 0x22, 77, 6), (0x40000, 0x1e, 67, 0x15)]),
        first(&[
            (0x400, 0x19, 59, 0xf),
            (0x100, 0x18, 59, 0xf),
            (0x200, 0x1a, 59, 0xf),
            (0x80_0000, 0x1b, 59, 0xf),
            (0x8000, 0xc, 79, 7),
            (0x8_0000, 0x1f, 69, 0x16),
        ]),
        first(&[
            (0x80, 0x17, 57, 0xe),
            (0x40, 0x16, 57, 0xd),
            (0x10_0000, 0x20, 71, 0x17),
            (0x200_0000, 0x23, 85, 0x6b),
        ]),
        first(&[(0x20_0000, 0x11, 83, 0x14), (0x1_0000, 0xf, 63, 0x12), (0x2000, 0x24, 75, 5)]),
        first(&[(0x2_0000, 0x10, 65, 0x13)]),
        first(&[(4, HALT, 61, 4)]),
    ]
}

/// State of TENIR LA POSITION or SUSPENDRE LE FEU over the selected units' flags (`0x10080fa0`):
/// 0 when they all have it, 1 when some do, 2 when none does.
pub fn hold_state(flags: impl IntoIterator<Item = bool>) -> usize {
    let (mut all, mut any) = (true, false);
    for f in flags {
        all &= f;
        any |= f;
    }
    if all {
        0
    } else if any {
        1
    } else {
        2
    }
}

/// What a press of the grid button for `command` does (`0x10080150`): HALTE acts at once, the
/// orders that need a target are armed, or disarmed when they already were.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Halt,
    Arm(u8),
    Disarm,
}

pub fn press(command: u8, armed: Option<u8>) -> Press {
    match command {
        HALT => Press::Halt,
        c if armed == Some(c) => Press::Disarm,
        c => Press::Arm(c),
    }
}

/// TENIR LA POSITION and SUSPENDRE LE FEU set the flag on all the selected units unless they all
/// have it already, then clear it (orders 0x55, 0x54 with `![0x103880c4]`, `![0x103880c8]`).
pub fn toggled(state: usize) -> bool {
    state != 0
}
