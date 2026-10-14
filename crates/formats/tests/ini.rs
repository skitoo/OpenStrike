use openstrike_formats::ini::Ini;

const SUDTEST: &str = "[StartUp]\r\nModule1=Menu_Dll.dll\r\n[Game]\r\nDataPathScm0_0=common.sue;units.sue;animats.sue;desc_common.sue\r\n\
DataPathScm1_0=summer.sue;desc_summer.sue\r\nDataPathScm3_0=beach.sue;summer.sue;desc_summer.sue\r\nDataPathScm0_1=common.sue\r\n\
DataPathScm1_1=summer.sue\r\nDataPathScm3_2=summer.sue\r\n";

#[test]
fn keys_and_sections_ignore_case() {
    let ini = Ini::parse(SUDTEST);
    assert_eq!(ini.get("startup", "MODULE1"), Some("Menu_Dll.dll"));
    assert_eq!(ini.get("Game", "Module1"), None);
}

#[test]
fn data_paths_follow_the_numbered_keys_up_to_the_first_gap() {
    let ini = Ini::parse(SUDTEST);
    assert_eq!(ini.data_paths(0), ["common.sue", "units.sue", "animats.sue", "desc_common.sue", "common.sue"]);
    assert_eq!(ini.data_paths(1), ["summer.sue", "desc_summer.sue", "summer.sue"]);
    // DataPathScm3_1 is missing, so DataPathScm3_2 is never read
    assert_eq!(ini.data_paths(3), ["beach.sue", "summer.sue", "desc_summer.sue"]);
    assert!(ini.data_paths(2).is_empty());
}
