//! OpenCode ve DeepSeek Harness yerel sayaçları: sentetik fikstürlerle (gerçek kullanıcı verisi yok).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use cetvel_baglayici::yerel_sayac::{SayimHatasi, dsh_say, opencode_say};
use cetvel_baglayici::{YerelArac, yerel_arac_var_mi, yerel_sayac_oku};
use cetvel_model::{Durum, GuvenEtiketi, KaynakTuru, UtcZaman};
use rusqlite::{Connection, params};

const GIZLI_BASLIK: &str = "GIZLI-BASLIK-proje-sirri";
const GIZLI_ISTEM: &str = "GIZLI-ISTEM-hasta-verisi";

fn gecici(ad: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cetvel-yerel-sayac-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("gecici");
    d
}

fn simdi() -> UtcZaman {
    UtcZaman::simdi()
}

fn simdi_ms() -> i64 {
    simdi().as_offset().unix_timestamp() * 1000
}

const SAAT_MS: i64 = 3_600_000;

fn zaman_ayarla(yol: &Path, once: Duration) {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(yol)
        .expect("zaman icin ac");
    f.set_modified(SystemTime::now() - once).expect("mtime");
}

/// Yayımlanan OpenCode sürümlerinin şeması: `session` tablosunda belirteç sütunları.
fn opencode_guncel_db(dizin: &Path) {
    let b = Connection::open(dizin.join("opencode.db")).expect("db");
    b.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE session (id TEXT PRIMARY KEY, project_id TEXT NOT NULL, title TEXT,
           directory TEXT NOT NULL, cost REAL NOT NULL DEFAULT 0,
           tokens_input INTEGER NOT NULL DEFAULT 0, tokens_output INTEGER NOT NULL DEFAULT 0,
           tokens_reasoning INTEGER NOT NULL DEFAULT 0, tokens_cache_read INTEGER NOT NULL DEFAULT 0,
           tokens_cache_write INTEGER NOT NULL DEFAULT 0,
           time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL);
         CREATE TABLE credential (id TEXT, secret TEXT);",
    )
    .expect("sema");
    let n = simdi_ms();
    for (id, guncel, girdi, cikti, akil, onbellek) in [
        ("ses_a", n - SAAT_MS, 1000i64, 200i64, 50i64, 90_000i64),
        ("ses_b", n - 30 * SAAT_MS, 300, 100, 0, 5),
        ("ses_c", n - 10 * 24 * SAAT_MS, 7000, 7000, 7000, 0),
    ] {
        b.execute(
            "INSERT INTO session VALUES (?1, 'p', ?2, 'C:/proje', 0.1, ?3, ?4, ?5, ?6, 0, ?7, ?7)",
            params![id, GIZLI_BASLIK, girdi, cikti, akil, onbellek, guncel],
        )
        .expect("satir");
    }
    b.execute(
        "INSERT INTO credential VALUES ('x', 'sk-gizli-anahtar')",
        [],
    )
    .expect("kimlik");
}

#[test]
fn opencode_guncel_db_oturum_ve_belirteci_sayar_onbellegi_ve_gizliyi_almaz() {
    let d = gecici("oc-guncel");
    opencode_guncel_db(&d);
    let s = opencode_say(&d, simdi().as_offset().unix_timestamp()).expect("sayim");
    assert_eq!((s.oturum_24s, s.oturum_7g, s.oturum_toplam), (1, 2, 3));
    assert_eq!(s.belirtec_24s, Some(1250));
    assert_eq!(s.belirtec_7g, Some(1650));
    assert!(s.son_etkinlik_sn.is_some());

    let havuz = yerel_sayac_oku(YerelArac::OpenCode, Some(&d), simdi());
    assert_eq!(havuz.durum, Durum::Ok);
    assert_eq!(havuz.kullanilabilir, Some(true));
    assert!(havuz.pencereler.is_empty(), "kota penceresi uydurulmaz");
    let k = havuz
        .ek
        .as_ref()
        .and_then(|e| e.kullanim.as_ref())
        .expect("sayac");
    assert!(k.kota_yok);
    let kaynak = havuz.kaynak.as_ref().expect("kaynak");
    assert_eq!(kaynak.tur, KaynakTuru::YerelGunluk);
    assert_eq!(kaynak.guven, GuvenEtiketi::YerelKayit);
    let json = serde_json::to_string(&havuz).unwrap();
    for gizli in [GIZLI_BASLIK, "sk-gizli", "C:/proje", "ses_a"] {
        assert!(!json.contains(gizli), "çıktıda {gizli} olmamalı: {json}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Eski bir oturuma bugün tek ileti atılması: oturumun ömür boyu belirteci "son 24 sa"ya yazılmaz.
#[test]
fn opencode_eski_oturumun_birikmis_belirteci_pencereye_yazilmaz() {
    let d = gecici("oc-eski-oturum");
    opencode_guncel_db(&d);
    let b = Connection::open(d.join("opencode.db")).expect("db");
    let n = simdi_ms();
    // 21 gün önce açılmış, 5 milyon belirteç harcamış, bugün güncellenmiş oturum.
    b.execute(
        "INSERT INTO session VALUES ('ses_eski', 'p', 't', 'C:/proje', 9.0, 3000000, 2000000, 0, 0, 0, ?1, ?2)",
        params![n - 21 * 24 * SAAT_MS, n - SAAT_MS],
    )
    .unwrap();
    drop(b);
    let s = opencode_say(&d, simdi().as_offset().unix_timestamp()).expect("sayim");
    assert_eq!((s.oturum_24s, s.oturum_7g, s.oturum_toplam), (2, 3, 4));
    assert_eq!(s.belirtec_24s, Some(1250), "5 milyon pencereye yazılmamalı");
    assert_eq!(s.belirtec_7g, Some(1650));
    assert!(s.not.as_deref().is_some_and(|n| n.contains("eksik")));
    let _ = std::fs::remove_dir_all(&d);
}

/// İleti tablosu varsa belirteç ileti zamanından sayılır (oturum toplamı kullanılmaz).
#[test]
fn opencode_ileti_tablosu_varsa_belirtec_ileti_zamanindan_sayilir() {
    let d = gecici("oc-ileti-oncelik");
    opencode_guncel_db(&d);
    let b = Connection::open(d.join("opencode.db")).expect("db");
    let n = simdi_ms();
    b.execute_batch(
        "CREATE TABLE message (id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);",
    )
    .unwrap();
    b.execute(
        "INSERT INTO session VALUES ('ses_eski', 'p', 't', 'C:/proje', 9.0, 3000000, 2000000, 0, 0, 0, ?1, ?2)",
        params![n - 21 * 24 * SAAT_MS, n - SAAT_MS],
    )
    .unwrap();
    for (zaman, veri) in [
        (
            n - SAAT_MS,
            r#"{"role":"assistant","tokens":{"input":800,"output":20}}"#,
        ),
        (
            n - 2 * 24 * SAAT_MS,
            r#"{"role":"assistant","tokens":{"input":100}}"#,
        ),
        (
            n - 20 * 24 * SAAT_MS,
            r#"{"role":"assistant","tokens":{"input":4000000}}"#,
        ),
    ] {
        b.execute(
            "INSERT INTO message VALUES ('m', 'ses_eski', ?1, ?1, ?2)",
            params![zaman, veri],
        )
        .unwrap();
    }
    drop(b);
    let s = opencode_say(&d, simdi().as_offset().unix_timestamp()).expect("sayim");
    assert_eq!(s.belirtec_24s, Some(820));
    assert_eq!(s.belirtec_7g, Some(920));
    assert!(s.not.is_none());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn opencode_dev_session_v2_tablosu_da_okunur() {
    let d = gecici("oc-v2");
    let b = Connection::open(d.join("opencode.db")).expect("db");
    b.execute_batch(
        "CREATE TABLE session_v2 (id TEXT, title TEXT, tokens_input INTEGER, tokens_output INTEGER,
           tokens_reasoning INTEGER, time_created INTEGER, time_updated INTEGER);",
    )
    .unwrap();
    b.execute(
        "INSERT INTO session_v2 VALUES ('s', 't', 5, 6, 7, ?1, ?1)",
        params![simdi_ms()],
    )
    .unwrap();
    drop(b);
    let s = opencode_say(&d, simdi().as_offset().unix_timestamp()).expect("sayim");
    assert_eq!((s.oturum_24s, s.oturum_toplam), (1, 1));
    assert_eq!(s.belirtec_24s, Some(18));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn opencode_eski_db_belirteci_ileti_jsonundan_toplar() {
    let d = gecici("oc-eski-db");
    let b = Connection::open(d.join("opencode.db")).expect("db");
    b.execute_batch(
        "CREATE TABLE session (id TEXT, title TEXT, time_created INTEGER, time_updated INTEGER);
         CREATE TABLE message (id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);",
    )
    .unwrap();
    let n = simdi_ms();
    b.execute(
        "INSERT INTO session VALUES ('s', ?1, ?2, ?2)",
        params![GIZLI_BASLIK, n],
    )
    .unwrap();
    for (zaman, veri) in [
        (n - SAAT_MS, format!(r#"{{"role":"user","content":"{GIZLI_ISTEM}"}}"#)),
        (
            n - 2 * SAAT_MS,
            r#"{"role":"assistant","tokens":{"input":40,"output":2,"reasoning":0,"cache":{"read":9999,"write":0}}}"#.to_string(),
        ),
        (
            n - 3 * 24 * SAAT_MS,
            r#"{"role":"assistant","tokens":{"input":100,"output":0}}"#.to_string(),
        ),
        (
            n - 9 * 24 * SAAT_MS,
            r#"{"role":"assistant","tokens":{"input":5000,"output":0}}"#.to_string(),
        ),
    ] {
        b.execute(
            "INSERT INTO message VALUES ('m', 's', ?1, ?1, ?2)",
            params![zaman, veri],
        )
        .unwrap();
    }
    drop(b);
    let s = opencode_say(&d, simdi().as_offset().unix_timestamp()).expect("sayim");
    assert_eq!(s.belirtec_24s, Some(42));
    assert_eq!(s.belirtec_7g, Some(142));
    let json =
        serde_json::to_string(&yerel_sayac_oku(YerelArac::OpenCode, Some(&d), simdi())).unwrap();
    assert!(!json.contains(GIZLI_ISTEM) && !json.contains(GIZLI_BASLIK));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn opencode_eski_json_depolama_ve_auth_dosyasi_okunmaz() {
    let d = gecici("oc-json");
    let oturum = d.join("storage").join("session").join("proje1");
    let ileti = d.join("storage").join("message").join("ses_1");
    std::fs::create_dir_all(&oturum).unwrap();
    std::fs::create_dir_all(&ileti).unwrap();
    // auth.json bozuk: okunmaya çalışılsa bile sonuç değişmemeli; asıl güvence kodda açılmaması.
    std::fs::write(d.join("auth.json"), "{\"anahtar\":\"sk-gizli\"}").unwrap();
    std::fs::write(
        oturum.join("ses_1.json"),
        format!("{{\"title\":\"{GIZLI_BASLIK}\"}}"),
    )
    .unwrap();
    std::fs::write(oturum.join("ses_2.json"), "{}").unwrap();
    zaman_ayarla(&oturum.join("ses_2.json"), Duration::from_secs(3 * 86_400));
    std::fs::write(
        ileti.join("msg_1.json"),
        r#"{"role":"assistant","tokens":{"input":7,"output":3}}"#,
    )
    .unwrap();
    std::fs::write(
        ileti.join("msg_2.json"),
        r#"{"role":"assistant","tokens":{"input":1}}"#,
    )
    .unwrap();
    zaman_ayarla(&ileti.join("msg_2.json"), Duration::from_secs(2 * 86_400));

    let s = opencode_say(&d, simdi().as_offset().unix_timestamp()).expect("sayim");
    assert_eq!((s.oturum_24s, s.oturum_7g, s.oturum_toplam), (1, 2, 2));
    assert_eq!(s.belirtec_24s, Some(10));
    assert_eq!(s.belirtec_7g, Some(11));
    assert!(yerel_arac_var_mi(
        YerelArac::OpenCode,
        Some(d.to_str().unwrap())
    ));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn kurulu_degilse_baglanmadi_ve_var_degil() {
    let d = gecici("yok");
    assert_eq!(opencode_say(&d, 0), Err(SayimHatasi::Yok));
    assert_eq!(dsh_say(&d, 0), Err(SayimHatasi::Yok));
    assert!(!yerel_arac_var_mi(
        YerelArac::OpenCode,
        Some(d.to_str().unwrap())
    ));
    assert!(!yerel_arac_var_mi(
        YerelArac::Dsh,
        Some(d.to_str().unwrap())
    ));
    let h = yerel_sayac_oku(YerelArac::Dsh, Some(&d), simdi());
    assert_eq!(h.durum, Durum::Baglanmadi);
    assert!(h.ek.is_none());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn bozuk_opencode_db_hata_doner_panik_yok() {
    let d = gecici("oc-bozuk");
    std::fs::write(d.join("opencode.db"), b"bu bir sqlite dosyasi degil").unwrap();
    let h = yerel_sayac_oku(YerelArac::OpenCode, Some(&d), simdi());
    assert_eq!(h.durum, Durum::Hata);
    assert!(h.hata.is_some());
    let _ = std::fs::remove_dir_all(&d);
}

fn dsh_oturumu(kok: &Path, proje: &str, oturum: &str, dosya: &str, icerik: &[u8]) -> PathBuf {
    let k = kok.join("sessions").join(proje).join(oturum);
    std::fs::create_dir_all(&k).unwrap();
    let yol = k.join(dosya);
    std::fs::write(&yol, icerik).unwrap();
    yol
}

#[test]
fn dsh_ham_gunlukte_usage_sayilir_parca_olaylari_cift_sayilmaz() {
    let d = gecici("dsh-ham");
    let n = simdi_ms();
    let gunluk = format!(
        "{{\"type\":\"session\",\"version\":3,\"id\":\"x\",\"createdAt\":{n},\"cwd\":\"C:/gizli-proje\"}}\n\
         {{\"type\":\"user/message\",\"seq\":1,\"time\":{t1},\"data\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{GIZLI_ISTEM}\"}}]}}}}\n\
         {{\"type\":\"assistant/chunk\",\"seq\":2,\"time\":{t1},\"data\":{{\"chunk\":{{\"type\":\"usage\",\"usage\":{{\"inputTokens\":30,\"outputTokens\":4}}}}}}}}\n\
         {{\"type\":\"assistant/message\",\"seq\":3,\"time\":{t1},\"data\":{{\"usage\":{{\"inputTokens\":30,\"outputTokens\":4}}}}}}\n\
         {{\"type\":\"assistant/message\",\"seq\":4,\"time\":{t2},\"data\":{{\"usage\":{{\"inputTokens\":100,\"outputTokens\":0}}}}}}\n\
         yarım satır {{",
        t1 = n - SAAT_MS,
        t2 = n - 3 * 24 * SAAT_MS
    );
    dsh_oturumu(
        &d,
        "--C-proje--",
        "o1",
        "session.v3.jsonl",
        gunluk.as_bytes(),
    );
    // Daha eski kuşak aynı klasörde: yalnız en yüksek kuşak sayılır.
    dsh_oturumu(
        &d,
        "--C-proje--",
        "o1",
        "session.jsonl",
        b"{\"type\":\"session\"}\n",
    );
    let eski = dsh_oturumu(&d, "--C-proje--", "o2", "session.jsonl", b"{}\n");
    zaman_ayarla(&eski, Duration::from_secs(20 * 86_400));

    let s = dsh_say(&d, simdi().as_offset().unix_timestamp()).expect("sayim");
    assert_eq!((s.oturum_24s, s.oturum_7g, s.oturum_toplam), (1, 1, 2));
    assert_eq!(s.belirtec_24s, Some(34));
    assert_eq!(s.belirtec_7g, Some(134));
    assert!(s.not.is_none());

    let h = yerel_sayac_oku(YerelArac::Dsh, Some(&d), simdi());
    assert_eq!(h.durum, Durum::Ok);
    assert_eq!(h.etiket, "DeepSeek Harness");
    let json = serde_json::to_string(&h).unwrap();
    assert!(!json.contains(GIZLI_ISTEM) && !json.contains("gizli-proje"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn dsh_sikistirilmis_gunlukte_yalniz_oturum_sayilir() {
    let d = gecici("dsh-zstd");
    dsh_oturumu(
        &d,
        "_no-cwd",
        "o1",
        "session.v3.jsonl.zstd",
        &[0x28, 0xb5, 0x2f, 0xfd, 0, 0],
    );
    dsh_oturumu(
        &d,
        "_no-cwd",
        "o2",
        "session.jsonl.zstd",
        &[0x28, 0xb5, 0x2f, 0xfd],
    );
    // Tanınmayan dosya oturum sayılmaz.
    dsh_oturumu(&d, "_no-cwd", "o3", "notlar.txt", b"x");
    let s = dsh_say(&d, simdi().as_offset().unix_timestamp()).expect("sayim");
    assert_eq!((s.oturum_24s, s.oturum_toplam), (2, 2));
    assert_eq!(
        s.belirtec_24s, None,
        "sıkıştırılmış günlükte belirteç uydurulmaz"
    );
    assert!(s.not.is_some());
    assert!(yerel_arac_var_mi(YerelArac::Dsh, Some(d.to_str().unwrap())));
    let _ = std::fs::remove_dir_all(&d);
}
