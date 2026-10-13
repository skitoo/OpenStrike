use openstrike::options::{Parsed, parse};

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

fn run(a: &[&str]) -> openstrike::options::Options {
    match parse(args(a)).unwrap() {
        Parsed::Run(o) => o,
        Parsed::Help => panic!("help"),
    }
}

#[test]
fn defaults_to_the_sample_map() {
    let o = run(&[]);
    assert!(o.map.ends_with("Maps/single/'Act of Sabotage'.ssm"));
    assert_eq!(o.climate, None);
}

#[test]
fn parses_every_option() {
    let o = run(&["--data", "/g", "--climate", "Winter", "--zoom", "1.5", "--look-at", "10, 20", "--show-blocked", "m.ssm"]);
    assert_eq!(o.data.to_str(), Some("/g"));
    assert_eq!(o.climate.as_deref(), Some("winter"));
    assert_eq!((o.zoom, o.look_at, o.show_blocked), (1.5, Some((10, 20)), true));
    assert_eq!(o.map.to_str(), Some("m.ssm"));
}

#[test]
fn typos_and_bad_values_are_errors() {
    assert!(parse(args(&["--zom", "2"])).unwrap_err().contains("unknown option"));
    assert!(parse(args(&["--zoom", "x"])).is_err());
    assert!(parse(args(&["--climate", "spring"])).is_err());
    assert!(parse(args(&["--look-at", "1"])).is_err());
    assert!(parse(args(&["--data"])).is_err());
    assert!(parse(args(&["a.ssm", "b.ssm"])).is_err());
    assert_eq!(parse(args(&["--help"])).unwrap(), Parsed::Help);
}
