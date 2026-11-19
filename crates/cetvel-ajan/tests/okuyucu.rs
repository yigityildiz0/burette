//! Sentetik fikstürlerle (rollout JSONL, özet SQLite'ı) uçtan uca okuyucu testi.

use std::io::Write;
use std::path::{Path, PathBuf};

use cetvel_ajan::{
    AjanAnlik, AjanDurumu, AjanOkuyucu, Etkinlik, HataTuru, KaynakDurumu, Kurallar, Platform,
    Yapilandirma, kimlik_karma,
};
use cetvel_model::UtcZaman;
use rusqlite::Connection;
use serde_json::json;

/// Fikstürlerde geçen ve çıktıya asla sızmaması gereken metinler.
const GIZLI: [&str; 6] = [
    "GIZLI_ISTEM_METNI",
    "cargo test --gizli-bayrak",
    "GIZLI_YANIT",
    "C:\\\\Kullanici\\\\proje",
    "019a-ust-thread",
    "agy-kok-konusma",
];

fn gecici_kok(ad: &str) -> PathBuf {
    let kok = std::env::temp_dir().join(format!("cetvel-ajan-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    std::fs::create_dir_all(&kok).unwrap();
    kok
}

fn satir_ekle(yol: &Path, deger: serde_json::Value) {
    let mut d = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(yol)
        .unwrap();
    writeln!(d, "{deger}").unwrap();
}

fn gizlilik_denetle(anlik: &AjanAnlik) {
    let metin = serde_json::to_string(anlik).unwrap();
    for g in GIZLI {
        assert!(!metin.contains(g), "çıktıda gizli metin var: {g}");
    }
    assert!(!metin.contains("Kullanici"), "yol sızdı");
}

fn codex_yapilandirma(kok: &Path) -> Yapilandirma {
    Yapilandirma {
        codex_oturum_koku: Some(kok.join("sessions")),
        ..Default::default()
    }
}

#[test]
fn codex_ebeveyn_alt_ajan_etkinlik_bitis_ve_hata() {
    let kok = gecici_kok("codex");
    let simdi = UtcZaman::simdi();
    let tarih = simdi.as_offset().date();
    let gun = kok
        .join("sessions")
        .join(format!("{:04}", tarih.year()))
        .join(format!("{:02}", u8::from(tarih.month())))
        .join(format!("{:02}", tarih.day()));
    std::fs::create_dir_all(&gun).unwrap();
    // Codex kökündeki kimlik dosyası hiçbir zaman okunmamalı (adı rollout değil).
    std::fs::write(kok.join("auth.json"), b"{\"tokens\":\"GIZLI\"}").unwrap();
    std::fs::write(kok.join("sessions").join("auth.json"), b"gizli").unwrap();

    let t = |sn: i64| simdi.ekle_sn(sn - 60).to_iso_z();
    let ust = gun.join("rollout-2026-11-16T10-00-00-ust.jsonl");
    satir_ekle(
        &ust,
        json!({"timestamp": t(0), "type": "session_meta", "payload": {
            "id": "019a-ust-thread", "timestamp": t(0), "cwd": "C:\\Kullanici\\proje",
            "originator": "codex_exec", "source": "exec", "thread_source": "user",
            "base_instructions": {"text": "GIZLI_ISTEM_METNI"}}}),
    );
    satir_ekle(
        &ust,
        json!({"timestamp": t(1), "type": "turn_context", "payload": {
            "model": "gpt-test-1", "effort": "high", "cwd": "C:\\Kullanici\\proje"}}),
    );
    satir_ekle(
        &ust,
        json!({"timestamp": t(1), "type": "event_msg", "payload": {"type": "task_started", "turn_id": "x"}}),
    );
    satir_ekle(
        &ust,
        json!({"timestamp": t(2), "type": "response_item", "payload": {
            "type": "message", "role": "user", "content": [{"type": "input_text", "text": "GIZLI_ISTEM_METNI"}]}}),
    );
    satir_ekle(
        &ust,
        json!({"timestamp": t(3), "type": "response_item", "payload": {
            "type": "custom_tool_call", "name": "exec", "call_id": "c1", "input": "cargo test --gizli-bayrak"}}),
    );
    satir_ekle(
        &ust,
        json!({"timestamp": t(4), "type": "response_item", "payload": {
            "type": "custom_tool_call_output", "call_id": "c1", "output": "GIZLI_YANIT"}}),
    );

    let alt = gun.join("rollout-2026-11-16T10-00-05-alt.jsonl");
    satir_ekle(
        &alt,
        json!({"timestamp": t(5), "type": "session_meta", "payload": {
            "id": "019a-alt-thread", "timestamp": t(5), "thread_source": "subagent",
            "source": {"subagent": {"thread_spawn": {"parent_thread_id": "019a-ust-thread", "depth": 1,
                "agent_path": "C:\\Kullanici\\proje", "agent_nickname": "Euclid", "agent_role": "web-search-agent"}}}}}),
    );
    satir_ekle(
        &alt,
        json!({"timestamp": t(6), "type": "event_msg", "payload": {"type": "task_started"}}),
    );
    satir_ekle(
        &alt,
        json!({"timestamp": t(7), "type": "event_msg", "payload": {"type": "item_completed", "item": {
            "type": "CommandExecution", "command": ["powershell", "Get-Content", "C:\\Kullanici\\proje\\a.txt"],
            "parsed_cmd": [{"type": "read", "cmd": "Get-Content a.txt"}], "status": "completed",
            "aggregated_output": "GIZLI_YANIT", "exit_code": 0}}}),
    );
    // Yarım satır: satır sonu gelene kadar işlenmez.
    {
        let mut d = std::fs::OpenOptions::new().append(true).open(&alt).unwrap();
        write!(
            d,
            "{{\"timestamp\":\"{}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_comp",
            t(8)
        )
        .unwrap();
    }

    let mut okuyucu = AjanOkuyucu::yeni(codex_yapilandirma(&kok));
    let anlik = okuyucu.tara(simdi);
    gizlilik_denetle(&anlik);
    assert_eq!(anlik.kaynaklar["codex"].durum, KaynakDurumu::Izleniyor);
    assert_eq!(anlik.ajanlar.len(), 2, "{anlik:#?}");

    let ust_k = kimlik_karma(Platform::Codex, "019a-ust-thread");
    let u = anlik.ajanlar.iter().find(|a| a.kimlik == ust_k).unwrap();
    assert_eq!(u.ad, "Codex exec");
    assert_eq!(u.model.as_deref(), Some("gpt-test-1"));
    assert_eq!(u.efor.as_deref(), Some("high"));
    assert_eq!(u.durum, AjanDurumu::Calisiyor);
    assert_eq!(u.etkinlik, Some(Etkinlik::Test));
    assert_eq!(u.sayilar.arac, 1);
    assert_eq!(u.ebeveyn, None);

    let a = anlik.ajanlar.iter().find(|a| a.kimlik != ust_k).unwrap();
    assert_eq!(a.ebeveyn.as_deref(), Some(ust_k.as_str()));
    assert_eq!(a.derinlik, 1);
    assert_eq!(a.ad, "Euclid");
    assert_eq!(a.etkinlik, Some(Etkinlik::Okuma));
    assert_eq!(a.durum, AjanDurumu::Calisiyor);

    // Yarım satır tamamlanınca alt ajan biter; 60 sn sonra listeden düşer.
    {
        let mut d = std::fs::OpenOptions::new().append(true).open(&alt).unwrap();
        writeln!(d, "lete\",\"last_agent_message\":\"GIZLI_YANIT\"}}}}").unwrap();
    }
    satir_ekle(
        &ust,
        json!({"timestamp": t(9), "type": "response_item", "payload": {
            "type": "function_call", "namespace": "collaboration", "name": "spawn_agent", "arguments": "{}"}}),
    );
    let anlik = okuyucu.tara(simdi);
    gizlilik_denetle(&anlik);
    let a = anlik.ajanlar.iter().find(|a| a.kimlik != ust_k).unwrap();
    assert_eq!(a.durum, AjanDurumu::Bitti);
    assert!(a.bitis.is_some());
    let u = anlik.ajanlar.iter().find(|a| a.kimlik == ust_k).unwrap();
    assert_eq!(u.etkinlik, Some(Etkinlik::AltAjan));
    assert_eq!(u.sayilar.alt_ajan, 1);

    let sonra = simdi.ekle_sn(62);
    let anlik = okuyucu.tara(sonra);
    assert_eq!(anlik.ajanlar.len(), 1, "biten alt ajan düşmeli");

    // Kota hatası: üst ajan hata (429) olur.
    satir_ekle(
        &ust,
        json!({"timestamp": simdi.ekle_sn(61).to_iso_z(), "type": "event_msg", "payload": {
            "type": "error", "message": "You've hit your usage limit. GIZLI_YANIT"}}),
    );
    let anlik = okuyucu.tara(sonra);
    gizlilik_denetle(&anlik);
    let u = &anlik.ajanlar[0];
    assert_eq!(u.durum, AjanDurumu::Hata);
    assert_eq!(u.hata_turu, Some(HataTuru::HizSiniri));

    // Codex hata sonrası turu task_complete ile kapatır: hata ezilmemeli.
    satir_ekle(
        &ust,
        json!({"timestamp": simdi.ekle_sn(62).to_iso_z(), "type": "event_msg", "payload": {
            "type": "task_complete", "last_agent_message": null}}),
    );
    let anlik = okuyucu.tara(sonra);
    let u = &anlik.ajanlar[0];
    assert_eq!(u.durum, AjanDurumu::Hata, "task_complete hatayı ezmemeli");
    assert_eq!(u.hata_turu, Some(HataTuru::HizSiniri));

    // Uzun süre etkinliksiz çalışan ajan askıya düşer.
    satir_ekle(
        &ust,
        json!({"timestamp": simdi.ekle_sn(70).to_iso_z(), "type": "event_msg", "payload": {"type": "task_started"}}),
    );
    let anlik = okuyucu.tara(simdi.ekle_sn(70 + 601));
    assert_eq!(anlik.ajanlar[0].durum, AjanDurumu::Askida);

    let _ = std::fs::remove_dir_all(&kok);
}

fn agy_zaman(z: UtcZaman) -> String {
    z.to_iso_z()
        .replace('T', " ")
        .replace('Z', ".1234567+00:00")
}

fn ozet_db_olustur(yol: &Path) -> Connection {
    let c = Connection::open(yol).unwrap();
    c.pragma_update(None, "journal_mode", "wal").unwrap();
    c.execute_batch(
        "CREATE TABLE `conversation_summaries` (`conversation_id` text,`title` text NOT NULL DEFAULT \"\",
         `preview` text NOT NULL DEFAULT \"\",`step_count` integer NOT NULL DEFAULT 0,
         `last_modified_time` datetime NOT NULL,`workspace_uris` text NOT NULL,`status` text NOT NULL DEFAULT \"\",
         `source` text NOT NULL DEFAULT \"\",`project_id` text NOT NULL DEFAULT \"\",`agent_name` text NOT NULL DEFAULT \"\",
         `parent_conversation_id` text NOT NULL DEFAULT \"\",`nesting_depth` integer NOT NULL DEFAULT 0,
         `battle_id` text NOT NULL DEFAULT \"\",`winning_conversation_id` text NOT NULL DEFAULT \"\",
         `not_fully_idle` numeric NOT NULL DEFAULT false,`killed` numeric NOT NULL DEFAULT false,
         `last_user_input_time` datetime NOT NULL,`last_user_input_step_index` integer NOT NULL DEFAULT -1,
         `app_data_dir` text NOT NULL DEFAULT \"\",`raw_summary` blob,`group_id` text NOT NULL DEFAULT \"\",
         PRIMARY KEY (`conversation_id`));
         CREATE TABLE conversation_parents (conversation_id TEXT PRIMARY KEY, parent_id TEXT NOT NULL) WITHOUT ROWID;",
    )
    .unwrap();
    c
}

#[allow(clippy::too_many_arguments)]
fn ozet_ekle(
    c: &Connection,
    kimlik: &str,
    ebeveyn: &str,
    derinlik: i64,
    ad: &str,
    durum: &str,
    calisiyor: bool,
    olduruldu: bool,
    son: UtcZaman,
) {
    c.execute(
        "INSERT OR REPLACE INTO conversation_summaries (conversation_id, title, preview, step_count,
         last_modified_time, workspace_uris, status, agent_name, parent_conversation_id, nesting_depth,
         not_fully_idle, killed, last_user_input_time, raw_summary)
         VALUES (?1, 'GIZLI_ISTEM_METNI', 'GIZLI_YANIT', 7, ?2, 'file:///C:/Kullanici/proje', ?3, ?4, ?5, ?6, ?7, ?8, ?9, x'00')",
        rusqlite::params![
            kimlik,
            agy_zaman(son),
            durum,
            ad,
            ebeveyn,
            derinlik,
            calisiyor,
            olduruldu,
            agy_zaman(son.ekle_sn(-30)),
        ],
    )
    .unwrap();
}

#[test]
fn antigravity_ozet_db_salt_okunur_agac_ve_durumlar() {
    let kok = gecici_kok("agy");
    let db = kok.join("ayar-disi").join("conversation_summaries.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    let yazan = ozet_db_olustur(&db);
    let simdi = UtcZaman::simdi();
    ozet_ekle(
        &yazan,
        "agy-kok-konusma",
        "",
        0,
        "",
        "CASCADE_RUN_STATUS_IDLE",
        true,
        false,
        simdi,
    );
    ozet_ekle(
        &yazan,
        "agy-alt-1",
        "agy-kok-konusma",
        1,
        "research",
        "CASCADE_RUN_STATUS_RUNNING",
        false,
        false,
        simdi,
    );
    // Ebeveyn yalnız conversation_parents tablosunda.
    ozet_ekle(
        &yazan,
        "agy-alt-2",
        "",
        0,
        "image-generator",
        "CASCADE_RUN_STATUS_IDLE",
        false,
        true,
        simdi,
    );
    yazan
        .execute(
            "INSERT INTO conversation_parents VALUES ('agy-alt-2', 'agy-kok-konusma')",
            [],
        )
        .unwrap();
    // Pencere dışı (3 saat önce) konuşma alınmaz.
    ozet_ekle(
        &yazan,
        "agy-eski",
        "",
        0,
        "",
        "CASCADE_RUN_STATUS_IDLE",
        false,
        false,
        simdi.ekle_sn(-3 * 3600),
    );

    // Yazan bağlantı açık ve okuma işlemi sürerken okunur (WAL; kilitlemez).
    let tx_tutucu = Connection::open(&db).unwrap();
    tx_tutucu
        .execute_batch("BEGIN; SELECT count(*) FROM conversation_summaries;")
        .unwrap();

    let mut okuyucu = AjanOkuyucu::yeni(Yapilandirma {
        agy_ozet_db: vec![
            db.clone(),
            kok.join("yok").join("conversation_summaries.db"),
        ],
        ..Default::default()
    });
    let anlik = okuyucu.tara(simdi);
    gizlilik_denetle(&anlik);
    assert_eq!(
        anlik.kaynaklar["antigravity"].durum,
        KaynakDurumu::Izleniyor
    );
    assert_eq!(anlik.ajanlar.len(), 3, "{anlik:#?}");
    let kok_k = kimlik_karma(Platform::Antigravity, "agy-kok-konusma");
    let k = anlik.ajanlar.iter().find(|a| a.kimlik == kok_k).unwrap();
    assert_eq!(k.durum, AjanDurumu::Calisiyor);
    assert_eq!(k.ad, "Antigravity");
    assert_eq!(k.model, None, "özet DB kesin model vermez");
    assert_eq!(k.sayilar.adim, Some(7));
    let a1 = anlik.ajanlar.iter().find(|a| a.ad == "research").unwrap();
    assert_eq!(a1.ebeveyn.as_deref(), Some(kok_k.as_str()));
    assert_eq!(a1.durum, AjanDurumu::Calisiyor);
    let a2 = anlik
        .ajanlar
        .iter()
        .find(|a| a.ad == "image-generator")
        .unwrap();
    assert_eq!(a2.ebeveyn.as_deref(), Some(kok_k.as_str()));
    assert_eq!(a2.derinlik, 1);
    assert_eq!(a2.durum, AjanDurumu::Iptal);

    // Yazan hâlâ yazabiliyor (okuyucu kilit bırakmadı).
    tx_tutucu.execute_batch("COMMIT;").unwrap();
    ozet_ekle(
        &yazan,
        "agy-kok-konusma",
        "",
        0,
        "",
        "CASCADE_RUN_STATUS_IDLE",
        false,
        false,
        simdi.ekle_sn(5),
    );
    let anlik = okuyucu.tara(simdi.ekle_sn(6));
    let k = anlik.ajanlar.iter().find(|a| a.kimlik == kok_k).unwrap();
    assert_eq!(k.durum, AjanDurumu::Bitti);
    let anlik = okuyucu.tara(simdi.ekle_sn(80));
    assert!(
        anlik.ajanlar.iter().all(|a| a.kimlik != kok_k),
        "biten kök düşmeli"
    );

    drop(yazan);
    drop(tx_tutucu);
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn kaynak_yoksa_bulunamadi_kapaliysa_kapali() {
    let kok = gecici_kok("yok");
    let mut okuyucu = AjanOkuyucu::yeni(Yapilandirma {
        codex_oturum_koku: Some(kok.join("yok-sessions")),
        agy_ozet_db: vec![kok.join("yok.db")],
        kurallar: Kurallar::default(),
    });
    let anlik = okuyucu.tara(UtcZaman::simdi());
    assert_eq!(anlik.kaynaklar["codex"].durum, KaynakDurumu::Bulunamadi);
    assert_eq!(
        anlik.kaynaklar["antigravity"].durum,
        KaynakDurumu::Bulunamadi
    );
    assert!(!anlik.kaynaklar.contains_key("claude_dis"));
    assert!(anlik.ajanlar.is_empty());
    let metin = serde_json::to_value(&anlik).unwrap();
    assert_eq!(metin["surum"], 1);
    let _ = std::fs::remove_dir_all(&kok);
}
