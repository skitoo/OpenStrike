use openstrike::orders::{ADVANCE, HALT, Press, can, grid, hold_state, press, selection, tank, toggled};

fn commands(c: u32) -> Vec<Option<u8>> {
    grid(c).iter().map(|b| b.map(|b| b.command)).collect()
}

#[test]
fn a_tank_shows_advance_attack_assault_turn_and_halt() {
    let c = selection([tank(false, true)]);
    assert_eq!(c & can::SINGLE_PLAYER, can::SINGLE_PLAYER);
    assert_eq!(commands(c), [Some(0x13), Some(0x14), Some(0x15), Some(0x1d), None, None, None, None, Some(0xb)]);
    let g = grid(c);
    // AVANCER: frame 51, text "AVANCER (~key_00~)"
    assert_eq!((g[0].unwrap().frame, g[0].unwrap().text), (51, 11));
    assert_eq!((g[8].unwrap().frame, g[8].unwrap().text), (61, 5));
}

#[test]
fn an_immobilised_tank_can_only_attack_and_halt() {
    assert_eq!(
        commands(selection([tank(true, true)])),
        [None, Some(0x14), None, None, None, None, None, None, Some(0xb)]
    );
    assert_eq!(commands(selection([tank(true, false)])), [None, None, None, None, None, None, None, None, Some(0xb)]);
    assert_eq!(selection([]), 0);
}

#[test]
fn hold_buttons_show_all_some_or_none() {
    assert_eq!(hold_state([true, true]), 0);
    assert_eq!(hold_state([true, false]), 1);
    assert_eq!(hold_state([false, false]), 2);
    // pressing sets the flag unless all had it
    assert!(!toggled(0) && toggled(1) && toggled(2));
}

#[test]
fn halt_acts_at_once_and_other_orders_wait_for_a_target() {
    assert_eq!(press(HALT, None), Press::Halt);
    assert_eq!(press(ADVANCE, None), Press::Arm(ADVANCE));
    assert_eq!(press(ADVANCE, Some(ADVANCE)), Press::Disarm);
    assert_eq!(press(0x14, Some(ADVANCE)), Press::Arm(0x14));
}
