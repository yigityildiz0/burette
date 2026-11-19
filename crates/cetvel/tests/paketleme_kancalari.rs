//! NSIS kurulum kancalarının (`paketleme/nsis/kancalar.nsh`) yapısal korumaları.
//! Kurulum dosyası testte çalıştırılamaz; burada yalnız bozulması kolay sözleşmeler denetlenir.

use std::path::PathBuf;

fn kancalar() -> String {
    let yol = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../paketleme/nsis/kancalar.nsh");
    let ham = std::fs::read(&yol).expect("kancalar.nsh okunamadı");
    // makensis Türkçe metni yalnız BOM'lu UTF-8'de doğru okur.
    assert!(
        ham.starts_with(b"\xEF\xBB\xBF"),
        "kancalar.nsh UTF-8 BOM'suz"
    );
    String::from_utf8(ham[3..].to_vec()).expect("kancalar.nsh UTF-8 değil")
}

fn makro<'a>(metin: &'a str, ad: &str) -> &'a str {
    let bas = metin
        .find(&format!("!macro {ad}"))
        .unwrap_or_else(|| panic!("{ad} makrosu yok"));
    let son = metin[bas..].find("!macroend").expect("!macroend yok");
    &metin[bas..bas + son]
}

#[test]
fn kurulum_oncesi_kilitli_cetvel_exe_kopyalamadan_once_kenara_alinir() {
    let k = kancalar();
    let on = makro(&k, "NSIS_HOOK_PREINSTALL");
    let durdur = on.find("durdur'").expect("durdur çağrısı yok");
    let kenara = on
        .find("CetvelKilitliyseKenaraAl")
        .expect("kilitli kopya kenara alınmıyor");
    let kopya = on.find("File \"/oname=cetvel.exe\"").expect("File yok");
    assert!(
        durdur < kenara && kenara < kopya,
        "sıra durdur → kenara al → File olmalı"
    );
    assert!(on.contains("Delete \"$INSTDIR\\cetvel-eski-*.exe\""));
}

#[test]
fn kenara_alma_yalniz_kilitliyse_tasir_ve_kayitcilari_korur() {
    let k = kancalar();
    let m = makro(&k, "CetvelKilitliyseKenaraAl");
    assert!(m.contains("FileOpen $1 \"${DOSYA}\" a"));
    assert!(m.contains("Rename \"${DOSYA}\""));
    assert!(
        m.contains("FileClose $1"),
        "kilitsiz dosyada tutamaç kapanmalı"
    );
    assert_eq!(m.matches("Push $").count(), m.matches("Pop $").count());
}

#[test]
fn kaldirma_kullanimdaki_exeyi_klasor_disina_tasir_ve_artiklari_siler() {
    let k = kancalar();
    let son = makro(&k, "NSIS_HOOK_POSTUNINSTALL");
    let tasima = son
        .find("\"$TEMP\\cetvel-kaldirilan\"")
        .expect("kullanımdaki exe geçici klasöre taşınmıyor");
    let artik = son
        .find("Delete \"$INSTDIR\\cetvel-eski-*.exe\"")
        .expect("kenara alınmış kopyalar silinmiyor");
    let rmdir = son.find("RMDir \"$INSTDIR\"").expect("RMDir yok");
    assert!(tasima < rmdir && artik < rmdir);
}

fn depo(yol: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(yol)
}

#[test]
fn lisans_belgeleri_kurulur_kaldirmada_silinir_ve_lisans_sayfasi_tanimli() {
    let k = kancalar();
    let on = makro(&k, "NSIS_HOOK_PREINSTALL");
    for dosya in [
        "LICENSE-MIT.txt",
        "LICENSE-APACHE.txt",
        "THIRD-PARTY-NOTICES.txt",
    ] {
        assert!(
            on.contains(&format!("/oname={dosya}")),
            "{dosya} kurulmuyor"
        );
        let son = makro(&k, "NSIS_HOOK_POSTUNINSTALL");
        assert!(
            son.contains(&format!("Delete \"$INSTDIR\\{dosya}\"")),
            "{dosya} kaldırmada silinmiyor"
        );
    }
    for kaynak in ["LICENSE-MIT", "LICENSE-APACHE", "THIRD-PARTY-NOTICES.txt"] {
        assert!(depo(kaynak).is_file(), "{kaynak} depo kökünde yok");
    }

    let conf = std::fs::read_to_string(depo("crates/cetvel-masaustu/tauri.conf.json"))
        .expect("tauri.conf.json");
    let v: serde_json::Value = serde_json::from_str(&conf).expect("json");
    let lisans_dosyasi = v["bundle"]["licenseFile"]
        .as_str()
        .expect("licenseFile yok");
    let yol = depo("crates/cetvel-masaustu").join(lisans_dosyasi);
    let ham = std::fs::read(&yol).expect("lisans sayfası okunamadı");
    assert!(
        ham.starts_with(b"\xEF\xBB\xBF"),
        "NSIS lisans sayfası BOM'lu UTF-8 olmalı"
    );
    let metin = String::from_utf8(ham[3..].to_vec())
        .unwrap()
        .replace("\r\n", "\n");
    let mit = std::fs::read_to_string(depo("LICENSE-MIT"))
        .unwrap()
        .replace("\r\n", "\n");
    let apache = std::fs::read_to_string(depo("LICENSE-APACHE"))
        .unwrap()
        .replace("\r\n", "\n");
    assert!(
        metin.contains(mit.trim()),
        "lisans sayfası güncel MIT metnini içermeli"
    );
    assert!(
        metin.contains(apache.trim_matches('\n')),
        "lisans sayfası güncel Apache metnini içermeli"
    );
    assert_eq!(v["bundle"]["license"], "MIT OR Apache-2.0");
}
