//! Codex `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` okuyucusu.
//!
//! Yalnız `rollout-*.jsonl` adlı dosyalar açılır (`auth.json` ve diğer Codex dosyalarına dokunulmaz).
//! Komut, istem ve yanıt metni yalnız tür çıkarmak için bellekte okunur; kayda geçmez.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cetvel_model::UtcZaman;
use serde::Deserialize;
use serde_json::Value;

use crate::kayit::{AjanKaydi, Kurallar};
use crate::kuyruk::JsonlKuyruk;
use crate::model::{AjanDurumu, Etkinlik, Platform, kisa_ad, model_kimligi};
use crate::siniflandir::{hata_turu_metinden, komut_etkinligi};
use crate::{KaynakDurumu, OkumaSonucu};

/// Her turda bu kadar gün geriye kadar tarih klasörleri taranır (uzun süren oturum için pay).
const GUN_GERIYE: i64 = 3;

/// Daha eski tarih klasörleri bu aralıkla taranır: `codex resume` eski oturumun dosyasına
/// kendi (eski) tarih klasöründe ekleme yapar. Bulunan dosya sonra her turda izlenir.
const TAM_TARAMA_SN: i64 = 60;

/// Bir rollout dosyası mı (`rollout-*.jsonl`).
pub fn rollout_adi_mi(ad: &str) -> bool {
    ad.starts_with("rollout-") && ad.ends_with(".jsonl")
}

#[derive(Debug)]
struct CodexDosya {
    kuyruk: JsonlKuyruk,
    ajan: Option<AjanKaydi>,
}

/// Codex oturum klasörünü yoklayan okuyucu.
#[derive(Debug)]
pub struct CodexOkuyucu {
    oturum_koku: PathBuf,
    dosyalar: BTreeMap<PathBuf, CodexDosya>,
    /// Son tam taramada eski tarih klasörlerinde pencere içinde değişmiş bulunan dosyalar.
    eski_adaylar: Vec<PathBuf>,
    son_tam_tarama: Option<UtcZaman>,
}

impl CodexOkuyucu {
    /// `oturum_koku`: `<CODEX_HOME>\sessions`.
    pub fn yeni(oturum_koku: impl Into<PathBuf>) -> Self {
        Self {
            oturum_koku: oturum_koku.into(),
            dosyalar: BTreeMap::new(),
            eski_adaylar: Vec::new(),
            son_tam_tarama: None,
        }
    }

    /// Klasörü yoklar, değişen dosyaların yeni satırlarını işler.
    pub fn tara(&mut self, simdi: UtcZaman, kurallar: &Kurallar) -> OkumaSonucu {
        if !self.oturum_koku.is_dir() {
            self.dosyalar.clear();
            return OkumaSonucu::durum(KaynakDurumu::Bulunamadi);
        }
        let tam_tarama_zamani = self
            .son_tam_tarama
            .is_none_or(|t| !(0..TAM_TARAMA_SN).contains(&simdi.fark_sn(t)));
        if tam_tarama_zamani {
            self.eski_adaylar = eski_degisenler(&self.oturum_koku, simdi, kurallar.pencere_sn);
            self.son_tam_tarama = Some(simdi);
        }
        let mut adaylar = aday_dosyalar(&self.oturum_koku, simdi);
        adaylar.extend(self.eski_adaylar.iter().cloned());
        // Bir kez bulunan (devam ettirilen eski oturum dahil) dosya her turda yoklanır.
        adaylar.extend(self.dosyalar.keys().cloned());
        adaylar.sort();
        adaylar.dedup();
        let mut guncel: Vec<PathBuf> = Vec::new();
        for yol in adaylar {
            let Some(degisim) = std::fs::metadata(&yol)
                .and_then(|m| m.modified())
                .ok()
                .and_then(sistem_zamani)
            else {
                continue;
            };
            if simdi.fark_sn(degisim) <= kurallar.pencere_sn {
                guncel.push(yol);
            }
        }
        self.dosyalar.retain(|yol, _| guncel.contains(yol));

        let mut hata_var = false;
        for yol in guncel {
            let dosya = self
                .dosyalar
                .entry(yol.clone())
                .or_insert_with(|| CodexDosya {
                    kuyruk: JsonlKuyruk::yeni(&yol),
                    ajan: None,
                });
            // İlk okumada büyük dosya birkaç parçada okunur; aynı turda yakalanır (ara durum yayınlanmaz).
            for _ in 0..8 {
                if dosya
                    .kuyruk
                    .oku_isle(|satir| match satir {
                        Some(satir) => satir_isle(&mut dosya.ajan, satir, &yol),
                        None => dosya.ajan = None,
                    })
                    .is_err()
                {
                    hata_var = true;
                    break;
                }
                if !dosya.kuyruk.bekleyen_var() {
                    break;
                }
            }
        }

        let kayitlar = self
            .dosyalar
            .values()
            .filter_map(|d| d.ajan.clone())
            .collect();
        OkumaSonucu {
            durum: if hata_var && self.dosyalar.is_empty() {
                KaynakDurumu::Okunamadi
            } else {
                KaynakDurumu::Izleniyor
            },
            kayitlar,
        }
    }
}

fn sistem_zamani(t: std::time::SystemTime) -> Option<UtcZaman> {
    let sn = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    UtcZaman::from_unix_timestamp(i64::try_from(sn).ok()?)
}

type GunAnahtari = (i32, i32, i32);

/// Her turda taranan en eski gün klasörü (`simdi - GUN_GERIYE gün`).
fn yakin_gun_siniri(simdi: UtcZaman) -> GunAnahtari {
    let alt_sinir = simdi.ekle_sn(-GUN_GERIYE * 86_400).as_offset().date();
    (
        alt_sinir.year(),
        u8::from(alt_sinir.month()) as i32,
        alt_sinir.day() as i32,
    )
}

/// `YYYY/MM/DD` gün klasörlerini listeler; `dahil` yıl / yıl-ay / gün önekine göre budar.
fn gun_klasorleri(kok: &Path, dahil: impl Fn(GunAnahtari, u8) -> bool) -> Vec<PathBuf> {
    let mut sonuc = Vec::new();
    for (yil, yil_yolu) in sayisal_alt_klasorler(kok) {
        if !dahil((yil, 0, 0), 1) {
            continue;
        }
        for (ay, ay_yolu) in sayisal_alt_klasorler(&yil_yolu) {
            if !dahil((yil, ay, 0), 2) {
                continue;
            }
            for (gun, gun_yolu) in sayisal_alt_klasorler(&ay_yolu) {
                if dahil((yil, ay, gun), 3) {
                    sonuc.push(gun_yolu);
                }
            }
        }
    }
    sonuc
}

/// `YYYY/MM/DD` klasörlerinden son [`GUN_GERIYE`] günü ve kökteki düz rollout dosyalarını listeler.
fn aday_dosyalar(kok: &Path, simdi: UtcZaman) -> Vec<PathBuf> {
    let s = yakin_gun_siniri(simdi);
    let mut sonuc = Vec::new();
    rollout_ekle(kok, &mut sonuc);
    let gunler = gun_klasorleri(kok, |(y, a, g), duzey| match duzey {
        1 => y >= s.0,
        2 => (y, a) >= (s.0, s.1),
        _ => (y, a, g) >= s,
    });
    for gun in gunler {
        rollout_ekle(&gun, &mut sonuc);
    }
    sonuc
}

/// [`GUN_GERIYE`] günden eski klasörlerde son `pencere_sn` içinde değişmiş rollout dosyaları
/// (`codex resume` ile sürdürülen eski oturumlar). Seyrek çalışır ([`TAM_TARAMA_SN`]).
fn eski_degisenler(kok: &Path, simdi: UtcZaman, pencere_sn: i64) -> Vec<PathBuf> {
    let s = yakin_gun_siniri(simdi);
    let gunler = gun_klasorleri(kok, |(y, a, g), duzey| match duzey {
        1 => y <= s.0,
        2 => (y, a) <= (s.0, s.1),
        _ => (y, a, g) < s,
    });
    let mut dosyalar = Vec::new();
    for gun in gunler {
        rollout_ekle(&gun, &mut dosyalar);
    }
    dosyalar.retain(|yol| {
        std::fs::metadata(yol)
            .and_then(|m| m.modified())
            .ok()
            .and_then(sistem_zamani)
            .is_some_and(|d| simdi.fark_sn(d) <= pencere_sn)
    });
    dosyalar
}

fn sayisal_alt_klasorler(yol: &Path) -> Vec<(i32, PathBuf)> {
    let Ok(okuyucu) = std::fs::read_dir(yol) else {
        return Vec::new();
    };
    okuyucu
        .flatten()
        .filter(|g| g.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|g| {
            let ad = g.file_name().to_str()?.parse::<i32>().ok()?;
            Some((ad, g.path()))
        })
        .collect()
}

fn rollout_ekle(klasor: &Path, sonuc: &mut Vec<PathBuf>) {
    let Ok(okuyucu) = std::fs::read_dir(klasor) else {
        return;
    };
    for g in okuyucu.flatten() {
        if g.file_type().is_ok_and(|t| t.is_file())
            && g.file_name().to_str().is_some_and(rollout_adi_mi)
        {
            sonuc.push(g.path());
        }
    }
}

// ---------------------------------------------------------------- satır ayrıştırma

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Satir {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    tur: Option<String>,
    payload: Option<Yuk>,
}

/// Yalnız gereken alanlar; büyük çıktı/metin alanları bildirilmediği için ayrılmaz (bellek).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Yuk {
    #[serde(rename = "type")]
    tur: Option<Value>,
    id: Option<Value>,
    timestamp: Option<Value>,
    parent_thread_id: Option<Value>,
    source: Option<Value>,
    thread_source: Option<Value>,
    agent_nickname: Option<Value>,
    agent_role: Option<Value>,
    originator: Option<Value>,
    model: Option<Value>,
    effort: Option<Value>,
    thread_settings: Option<Value>,
    name: Option<Value>,
    namespace: Option<Value>,
    role: Option<Value>,
    input: Option<Value>,
    arguments: Option<Value>,
    action: Option<Value>,
    command: Option<Value>,
    exit_code: Option<Value>,
    success: Option<Value>,
    message: Option<Value>,
    codex_error_info: Option<Value>,
    item: Option<Oge>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Oge {
    #[serde(rename = "type")]
    tur: Option<Value>,
    status: Option<Value>,
    parsed_cmd: Option<Value>,
    command: Option<Value>,
    server: Option<Value>,
    kind: Option<Value>,
}

fn metin(v: &Option<Value>) -> Option<&str> {
    v.as_ref().and_then(Value::as_str)
}

/// Komut değerini (dizi ya da metin) tek metne çevirir (yalnız sınıflama için).
fn komut_metni(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn satir_isle(ajan: &mut Option<AjanKaydi>, satir: &str, yol: &Path) {
    let Ok(s) = serde_json::from_str::<Satir>(satir) else {
        return;
    };
    let Some(yuk) = s.payload else {
        return;
    };
    let zaman = s
        .timestamp
        .as_deref()
        .or(metin(&yuk.timestamp))
        .and_then(|t| UtcZaman::parse_tarih_veya_zaman(t).ok());

    if s.tur.as_deref() == Some("session_meta") {
        oturum_basi(ajan, &yuk, zaman, yol);
        return;
    }
    // session_meta görülmeden gelen satırlar (çok büyük dosyanın ortası) dosya adıyla kaydedilir.
    let a = ajan.get_or_insert_with(|| {
        let mut k = AjanKaydi::yeni(
            Platform::Codex,
            dosya_kimligi(yol),
            zaman.unwrap_or_else(UtcZaman::simdi),
        );
        k.ad = "Codex".to_string();
        k
    });
    let z = zaman.unwrap_or(a.son_etkinlik);

    match s.tur.as_deref() {
        Some("turn_context") => {
            if let Some(m) = metin(&yuk.model).and_then(model_kimligi) {
                a.model = Some(m);
            }
            if let Some(e) = metin(&yuk.effort).and_then(model_kimligi) {
                a.efor = Some(e);
            }
            a.zaman_isle(z);
        }
        Some("event_msg") => olay_isle(a, &yuk, z),
        Some("response_item") => yanit_ogesi_isle(a, &yuk, z),
        _ => a.zaman_isle(z),
    }
}

fn dosya_kimligi(yol: &Path) -> String {
    yol.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("rollout")
        .to_string()
}

fn oturum_basi(ajan: &mut Option<AjanKaydi>, yuk: &Yuk, zaman: Option<UtcZaman>, yol: &Path) {
    let kimlik = metin(&yuk.id)
        .map(str::to_string)
        .unwrap_or_else(|| dosya_kimligi(yol));
    let z = zaman.unwrap_or_else(UtcZaman::simdi);
    let a = ajan.get_or_insert_with(|| AjanKaydi::yeni(Platform::Codex, kimlik.clone(), z));
    a.yerel_kimlik = kimlik;
    a.zaman_isle(z);

    let dogum = yuk
        .source
        .as_ref()
        .and_then(|s| s.get("subagent"))
        .and_then(|s| s.get("thread_spawn"));
    let ebeveyn = metin(&yuk.parent_thread_id)
        .or_else(|| {
            dogum
                .and_then(|d| d.get("parent_thread_id"))
                .and_then(Value::as_str)
        })
        .filter(|e| !e.is_empty());
    a.ebeveyn_yerel = ebeveyn.map(str::to_string);
    a.derinlik = dogum
        .and_then(|d| d.get("depth"))
        .and_then(Value::as_u64)
        .map(|d| d as u32)
        .unwrap_or(u32::from(a.ebeveyn_yerel.is_some()));

    let takma = metin(&yuk.agent_nickname)
        .or_else(|| {
            dogum
                .and_then(|d| d.get("agent_nickname"))
                .and_then(Value::as_str)
        })
        .and_then(kisa_ad);
    let rol = metin(&yuk.agent_role)
        .or_else(|| {
            dogum
                .and_then(|d| d.get("agent_role"))
                .and_then(Value::as_str)
        })
        .and_then(kisa_ad);
    a.ad = if a.ebeveyn_yerel.is_some() || metin(&yuk.thread_source) == Some("subagent") {
        takma.or(rol).unwrap_or_else(|| "Alt ajan".to_string())
    } else {
        let kaynak = metin(&yuk.source).unwrap_or("").to_ascii_lowercase();
        let baslatan = metin(&yuk.originator).unwrap_or("").to_ascii_lowercase();
        if kaynak == "exec" || baslatan.contains("exec") {
            "Codex exec"
        } else if kaynak == "vscode" || baslatan.contains("vscode") {
            "Codex VS Code"
        } else if baslatan.contains("desktop") {
            "Codex masaüstü"
        } else if kaynak == "cli" || baslatan.contains("cli") {
            "Codex CLI"
        } else {
            "Codex"
        }
        .to_string()
    };
}

fn olay_isle(a: &mut AjanKaydi, yuk: &Yuk, z: UtcZaman) {
    let tur = metin(&yuk.tur).unwrap_or("");
    match tur {
        "task_started" | "turn_started" => {
            a.basladi(z);
            a.etkinlik = Some(Etkinlik::Dusunme);
        }
        "task_complete" | "turn_complete" => a.bitir(z, AjanDurumu::Bitti, None),
        "turn_aborted" => a.bitir(z, AjanDurumu::Iptal, None),
        "error" => {
            let mut ipucu = metin(&yuk.message).unwrap_or("").to_string();
            if let Some(bilgi) = &yuk.codex_error_info {
                ipucu.push(' ');
                ipucu.push_str(&bilgi.to_string());
            }
            a.tur_hatasi_isle(z, hata_turu_metinden(&ipucu));
        }
        "exec_approval_request"
        | "apply_patch_approval_request"
        | "request_user_input"
        | "elicitation_request" => {
            a.zaman_isle(z);
            if !a.durum.bitmis_mi() {
                a.durum = AjanDurumu::Bekliyor;
            }
        }
        "exec_command_begin" => {
            let e = yuk
                .command
                .as_ref()
                .map(|c| komut_etkinligi(&komut_metni(c)))
                .unwrap_or(Etkinlik::Komut);
            a.etkinlik_isle(z, e);
        }
        "exec_command_end" => {
            a.zaman_isle(z);
            if yuk
                .exit_code
                .as_ref()
                .and_then(Value::as_i64)
                .is_some_and(|k| k != 0)
            {
                a.sayilar.arac_hatasi += 1;
            }
        }
        "patch_apply_begin" => a.etkinlik_isle(z, Etkinlik::Yazma),
        "patch_apply_end" => {
            a.zaman_isle(z);
            if yuk.success.as_ref().and_then(Value::as_bool) == Some(false) {
                a.sayilar.arac_hatasi += 1;
            }
        }
        "web_search_begin" => a.etkinlik_isle(z, Etkinlik::Web),
        "mcp_tool_call_begin" => a.etkinlik_isle(z, Etkinlik::Arac),
        "view_image_tool_call" => a.etkinlik_isle(z, Etkinlik::Okuma),
        "agent_reasoning" | "agent_reasoning_raw_content" | "agent_reasoning_section_break" => {
            a.etkinlik_isle(z, Etkinlik::Dusunme)
        }
        "agent_message" => a.etkinlik_isle(z, Etkinlik::Yanit),
        t if t.starts_with("collab_") => a.etkinlik_isle(z, Etkinlik::AltAjan),
        "item_started" | "item_completed" => {
            if let Some(oge) = &yuk.item {
                oge_isle(a, oge, z);
            } else {
                a.zaman_isle(z);
            }
        }
        "thread_settings_applied" => {
            a.zaman_isle(z);
            if let Some(ayar) = &yuk.thread_settings {
                if let Some(m) = ayar
                    .get("model")
                    .and_then(Value::as_str)
                    .and_then(model_kimligi)
                {
                    a.model = Some(m);
                }
                if let Some(e) = ayar
                    .get("reasoning_effort")
                    .and_then(Value::as_str)
                    .and_then(model_kimligi)
                {
                    a.efor = Some(e);
                }
            }
        }
        _ => a.zaman_isle(z),
    }
}

fn oge_isle(a: &mut AjanKaydi, oge: &Oge, z: UtcZaman) {
    let durum = metin(&oge.status).unwrap_or("");
    if matches!(durum, "failed" | "declined" | "error") {
        a.sayilar.arac_hatasi += 1;
    }
    let e = match metin(&oge.tur).unwrap_or("") {
        "CommandExecution" | "command_execution" => {
            let turler: Vec<&str> = oge
                .parsed_cmd
                .as_ref()
                .and_then(Value::as_array)
                .map(|d| {
                    d.iter()
                        .filter_map(|p| p.get("type").and_then(Value::as_str))
                        .collect()
                })
                .unwrap_or_default();
            if turler.contains(&"search") {
                Etkinlik::Arama
            } else if !turler.is_empty()
                && turler.iter().all(|t| matches!(*t, "read" | "list_files"))
            {
                Etkinlik::Okuma
            } else {
                oge.command
                    .as_ref()
                    .map(|c| komut_etkinligi(&komut_metni(c)))
                    .unwrap_or(Etkinlik::Komut)
            }
        }
        "FileChange" | "file_change" => Etkinlik::Yazma,
        "McpToolCall" | "mcp_tool_call" => {
            let sunucu = metin(&oge.server).unwrap_or("").to_ascii_lowercase();
            if ["chrome", "browser", "playwright", "web", "fetch"]
                .iter()
                .any(|k| sunucu.contains(k))
            {
                Etkinlik::Web
            } else {
                Etkinlik::Arac
            }
        }
        "WebSearch" | "web_search" => Etkinlik::Web,
        "ImageView" | "image_view" => Etkinlik::Okuma,
        "Reasoning" | "reasoning" => Etkinlik::Dusunme,
        "AgentMessage" | "agent_message" => Etkinlik::Yanit,
        "TodoList" | "todo_list" | "Plan" => Etkinlik::Dusunme,
        "SubAgentActivity" | "CollabAgentToolCall" => Etkinlik::AltAjan,
        "Extension" => {
            let tur = metin(&oge.kind).unwrap_or("");
            if tur.starts_with("web.") {
                Etkinlik::Web
            } else if tur.starts_with("image") {
                Etkinlik::Gorsel
            } else if tur.starts_with("clock.") {
                Etkinlik::Bekleme
            } else {
                Etkinlik::Arac
            }
        }
        "UserMessage" | "user_message" => {
            a.zaman_isle(z);
            return;
        }
        _ => Etkinlik::Arac,
    };
    a.etkinlik_isle(z, e);
}

fn yanit_ogesi_isle(a: &mut AjanKaydi, yuk: &Yuk, z: UtcZaman) {
    match metin(&yuk.tur).unwrap_or("") {
        "reasoning" => a.etkinlik_isle(z, Etkinlik::Dusunme),
        "message" => {
            if metin(&yuk.role) == Some("assistant") {
                a.etkinlik_isle(z, Etkinlik::Yanit);
            } else {
                a.zaman_isle(z);
            }
        }
        "agent_message" => a.etkinlik_isle(z, Etkinlik::AltAjan),
        "function_call" => {
            a.sayilar.arac += 1;
            let ad = metin(&yuk.name).unwrap_or("");
            let ad_alani = metin(&yuk.namespace).unwrap_or("");
            match ad {
                "request_user_input" => {
                    a.zaman_isle(z);
                    if !a.durum.bitmis_mi() {
                        a.durum = AjanDurumu::Bekliyor;
                    }
                }
                _ => {
                    if ad == "spawn_agent" {
                        a.sayilar.alt_ajan += 1;
                    }
                    let e = islev_etkinligi(ad, ad_alani, yuk.arguments.as_ref());
                    a.etkinlik_isle(z, e);
                }
            }
        }
        "custom_tool_call" => {
            a.sayilar.arac += 1;
            let e = match metin(&yuk.name).unwrap_or("") {
                "exec" | "shell" | "exec_command" => yuk
                    .input
                    .as_ref()
                    .map(|i| komut_etkinligi(&komut_metni(i)))
                    .unwrap_or(Etkinlik::Komut),
                "apply_patch" => Etkinlik::Yazma,
                _ => Etkinlik::Arac,
            };
            a.etkinlik_isle(z, e);
        }
        "local_shell_call" => {
            a.sayilar.arac += 1;
            let e = yuk
                .action
                .as_ref()
                .and_then(|x| x.get("command"))
                .map(|c| komut_etkinligi(&komut_metni(c)))
                .unwrap_or(Etkinlik::Komut);
            a.etkinlik_isle(z, e);
        }
        "web_search_call" => {
            a.sayilar.arac += 1;
            a.etkinlik_isle(z, Etkinlik::Web);
        }
        "tool_search_call" => {
            a.sayilar.arac += 1;
            a.etkinlik_isle(z, Etkinlik::Arac);
        }
        _ => a.zaman_isle(z),
    }
}

fn islev_etkinligi(ad: &str, ad_alani: &str, argumanlar: Option<&Value>) -> Etkinlik {
    match ad {
        "shell" | "shell_command" | "exec_command" | "container.exec" | "local_shell"
        | "unified_exec" => argumanlar
            .and_then(Value::as_str)
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|v| v.get("command").or_else(|| v.get("cmd")).cloned())
            .map(|c| komut_etkinligi(&komut_metni(&c)))
            .unwrap_or(Etkinlik::Komut),
        "write_stdin" => Etkinlik::Komut,
        "apply_patch" => Etkinlik::Yazma,
        "update_plan" => Etkinlik::Dusunme,
        "view_image" => Etkinlik::Okuma,
        "web_search" | "search_web" => Etkinlik::Web,
        "spawn_agent" | "send_message" | "send_input" | "close_agent" | "resume_agent" => {
            Etkinlik::AltAjan
        }
        "wait" | "sleep" => Etkinlik::Bekleme,
        "send_message_to_user_async" => Etkinlik::Yanit,
        _ if ad_alani == "collaboration" => Etkinlik::AltAjan,
        _ if ad_alani == "web" => Etkinlik::Web,
        _ => Etkinlik::Arac,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eski_gun_klasoru_yalniz_tam_taramada_ve_pencere_icindeyse_aday() {
        let kok = std::env::temp_dir().join(format!("cetvel-codex-eski-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let simdi = UtcZaman::simdi();
        let gun_yolu = |z: UtcZaman| {
            let t = z.as_offset().date();
            kok.join(format!("{:04}", t.year()))
                .join(format!("{:02}", u8::from(t.month())))
                .join(format!("{:02}", t.day()))
        };
        let eski = gun_yolu(simdi.ekle_sn(-10 * 86_400));
        let yakin = gun_yolu(simdi);
        std::fs::create_dir_all(&eski).unwrap();
        std::fs::create_dir_all(&yakin).unwrap();
        let eski_dosya = eski.join("rollout-eski.jsonl");
        let meta = format!(
            "{{\"timestamp\":\"{}\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"eski-oturum\"}}}}
",
            simdi.to_iso_z()
        );
        std::fs::write(&eski_dosya, meta).unwrap();
        std::fs::write(
            yakin.join("rollout-yakin.jsonl"),
            b"{}
",
        )
        .unwrap();
        std::fs::write(eski.join("auth.json"), b"{}").unwrap();

        let yakinlar = aday_dosyalar(&kok, simdi);
        assert_eq!(yakinlar.len(), 1);
        assert!(!yakinlar.contains(&eski_dosya));
        // Eski klasördeki dosya az önce yazıldı (resume): tam taramada bulunur.
        assert_eq!(eski_degisenler(&kok, simdi, 7200), vec![eski_dosya.clone()]);
        let mut okuyucu = CodexOkuyucu::yeni(&kok);
        let sonuc = okuyucu.tara(simdi, &Kurallar::default());
        assert_eq!(
            sonuc.kayitlar.len(),
            1,
            "devam ettirilen eski oturum görünmeli"
        );
        assert_eq!(sonuc.kayitlar[0].yerel_kimlik, "eski-oturum");
        // Sonraki turlarda (tam tarama beklenmeden) izlenmeye devam eder.
        let sonuc = okuyucu.tara(simdi.ekle_sn(2), &Kurallar::default());
        assert_eq!(sonuc.kayitlar.len(), 1);
        // Pencere dışında kalan eski dosya aday olmaz.
        assert!(eski_degisenler(&kok, simdi.ekle_sn(3 * 3600), 7200).is_empty());
        let _ = std::fs::remove_dir_all(&kok);
    }

    #[test]
    fn auth_ve_diger_dosyalar_rollout_sayilmaz() {
        assert!(rollout_adi_mi("rollout-2026-11-16T10-00-00-abc.jsonl"));
        assert!(!rollout_adi_mi("auth.json"));
        assert!(!rollout_adi_mi("history.jsonl"));
        assert!(!rollout_adi_mi("rollout-x.json"));
    }
}
