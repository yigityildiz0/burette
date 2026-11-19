use std::collections::BTreeMap;

use cetvel_baglayici::{agy_cikti_ayristir, codex_yanit_ayristir};
use cetvel_model::{AnlikV2, Durum, UtcZaman, UyumDurum, anlik_dogrula, uyum_dogrula, uyum_uret};
use serde_json::Value;

fn z(s: &str) -> UtcZaman {
    UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
}

#[test]
fn fikstur_codex_haftalik_tukendi_anlik_ve_uyuma_dogru_yansir() {
    let ham = include_str!("../../../fixtures/codex-haftalik-tukendi.json");
    let val: Value = serde_json::from_str(ham).expect("codex fikstur json");
    let okuma = z("2026-11-15T18:00:00Z");

    let codex = codex_yanit_ayristir(&val, okuma);
    assert_eq!(codex.durum, Durum::Ok);
    assert_eq!(codex.pencereler["5s"].kalan, 1.0);
    assert!(codex.pencereler["5s"].sifirlama.is_none());
    assert_eq!(codex.pencereler["hafta"].kalan, 0.0);
    assert_eq!(codex.etkin_kalan, Some(0.0));
    assert_eq!(codex.baglayici_pencere.as_deref(), Some("hafta"));
    assert_eq!(codex.kullanilabilir, Some(false));

    let mut havuzlar = BTreeMap::new();
    havuzlar.insert("codex_plus".to_string(), codex);
    let anlik = AnlikV2::yeni(okuma, 1, havuzlar);
    anlik_dogrula(&anlik).expect("v2 dogrula");

    let uyum = uyum_uret(&anlik, z("2026-11-15T18:02:00Z"));
    uyum_dogrula(&uyum).expect("v1 uyum dogrula");
    let u_codex = &uyum.havuzlar["codex_plus"];
    assert_eq!(u_codex.durum, UyumDurum::Ok);
    assert_eq!(u_codex.etkin_kalan, Some(0.0));
    assert_eq!(u_codex.baglayici_pencere.as_deref(), Some("hafta"));
    assert!(!u_codex.kullanilabilir);
    assert!(u_codex.soguma_bitis.is_some());
}

#[test]
fn fikstur_codex_normal_baglayici_pencereyi_secer() {
    let ham = include_str!("../../../fixtures/codex-normal.json");
    let val: Value = serde_json::from_str(ham).expect("codex normal");
    let okuma = z("2026-11-15T18:00:00Z");

    let codex = codex_yanit_ayristir(&val, okuma);
    assert_eq!(codex.durum, Durum::Ok);
    assert!((codex.pencereler["5s"].kalan - 0.775).abs() < 1e-9);
    assert!((codex.pencereler["hafta"].kalan - 0.55).abs() < 1e-9);
    assert!((codex.etkin_kalan.unwrap() - 0.55).abs() < 1e-9);
    assert_eq!(codex.baglayici_pencere.as_deref(), Some("hafta"));
    assert_eq!(codex.kullanilabilir, Some(true));
}

#[test]
fn fikstur_agy_usage_metin_ciktisi_uc_havuz_ve_kalan_degerler_uretilir() {
    let ham = include_str!("../../../fixtures/agy-usage-metin.txt");
    let okuma = z("2026-11-16T16:50:00Z");

    let havuzlar = agy_cikti_ayristir(ham, okuma);
    assert_eq!(havuzlar.len(), 3);

    let gemini = &havuzlar["agy_gemini"];
    assert_eq!(gemini.durum, Durum::Ok);
    assert_eq!(gemini.pencereler.len(), 2);
    assert!((gemini.pencereler["hafta"].kalan - 0.23).abs() < 1e-9);
    assert!((gemini.pencereler["hafta"].kullanilan - 0.77).abs() < 1e-9);
    assert!((gemini.pencereler["5s"].kalan - 1.00).abs() < 1e-9);
    assert!((gemini.pencereler["5s"].kullanilan - 0.00).abs() < 1e-9);
    assert_eq!(gemini.baglayici_pencere.as_deref(), Some("hafta"));
    assert!((gemini.etkin_kalan.unwrap() - 0.23).abs() < 1e-9);

    let claude_gpt = &havuzlar["agy_claude_gpt"];
    assert_eq!(claude_gpt.durum, Durum::Ok);
    assert_eq!(claude_gpt.pencereler.len(), 2);
    assert!((claude_gpt.pencereler["hafta"].kalan - 0.47).abs() < 1e-9);
    assert!((claude_gpt.pencereler["hafta"].kullanilan - 0.53).abs() < 1e-9);
    assert!((claude_gpt.pencereler["5s"].kalan - 1.00).abs() < 1e-9);
    assert!((claude_gpt.pencereler["5s"].kullanilan - 0.00).abs() < 1e-9);
    assert_eq!(claude_gpt.baglayici_pencere.as_deref(), Some("hafta"));
    assert!((claude_gpt.etkin_kalan.unwrap() - 0.47).abs() < 1e-9);

    let argon = &havuzlar["agy_argon"];
    assert_eq!(argon.durum, Durum::Ok);
    assert_eq!(argon.pencereler.len(), 1);
    assert!(!argon.pencereler.contains_key("5s"));
    assert!((argon.pencereler["hafta"].kalan - 0.98).abs() < 1e-9);
    assert!((argon.pencereler["hafta"].kullanilan - 0.02).abs() < 1e-9);
    assert_eq!(argon.baglayici_pencere.as_deref(), Some("hafta"));
    assert!((argon.etkin_kalan.unwrap() - 0.98).abs() < 1e-9);

    let anlik = AnlikV2::yeni(okuma, 1, havuzlar);
    anlik_dogrula(&anlik).expect("v2 dogrula");

    let uyum = uyum_uret(&anlik, okuma);
    uyum_dogrula(&uyum).expect("v1 dogrula");
    assert_eq!(uyum.havuzlar.len(), 3);
    assert!((uyum.havuzlar["agy_gemini"].etkin_kalan.unwrap() - 0.23).abs() < 1e-9);
    assert!((uyum.havuzlar["agy_claude_gpt"].etkin_kalan.unwrap() - 0.47).abs() < 1e-9);
    assert!((uyum.havuzlar["agy_argon"].etkin_kalan.unwrap() - 0.98).abs() < 1e-9);
}
