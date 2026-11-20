//! Kotası olmayan yerel araçların kullanım sayacı: OpenCode ve DeepSeek Harness (`dsh`).
//!
//! İki araç da resmî bir kota ya da bakiye arayüzü sunmaz; Cetvel yalnız kendi yerel kayıtlarından
//! **sayar**: son 24 saatte ve son 7 günde etkin oturum, toplam oturum ve (kaynak veriyorsa)
//! belirteç. Havuz bir kota değildir: pencere/yüzde üretilmez, `ek.kullanim` doldurulur ve arayüz
//! "kota yok, kullanım sayacı" etiketini gösterir.
//!
//! Gizlilik: yalnız sayı ve zaman okunur. OpenCode'da `session`/`session_v2` tablosunun zaman ve
//! belirteç sütunları ile (eski sürümlerde) `message.data` içindeki `tokens` alanı ayrıştırılır;
//! başlık, istem, yanıt, dizin ve kimlik sütunları seçilmez, `auth.json` hiç açılmaz. dsh'de
//! yalnız `session*.jsonl[.zstd]` dosyalarının değişim zamanı ve (sıkıştırılmamış günlükte) olay
//! satırlarının `usage` sayıları okunur.

use std::collections::BTreeMap;
use std::future::Future;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, SystemTime};

use cetvel_depo::YerelSayacAyari;
use cetvel_model::{
    Durum, EkBilgi, GuvenEtiketi, HataBilgisi, HavuzAnlik, KaynakBilgisi, KaynakTuru,
    KullanimSayaci, UtcZaman,
};
use rusqlite::{Connection, OpenFlags, params};
use serde_json::Value;

use crate::{Baglayici, BaglayiciSonucu};

const GUN_SN: i64 = 86_400;
/// Eski JSON depolamasında en çok bu kadar ileti dosyası taranır (çok büyük klasörde donma olmasın).
const MAKS_ILETI_DOSYASI: usize = 20_000;
/// dsh'de en çok bu kadar oturum klasörü taranır.
const MAKS_DSH_OTURUM: usize = 5_000;
/// Sıkıştırılmamış dsh günlüğünde satır başına okunacak azami bayt (dev satırlar atlanır).
const MAKS_SATIR_BAYT: usize = 4 * 1024 * 1024;
/// Tek günlükten okunacak azami bayt.
const MAKS_GUNLUK_BAYT: u64 = 64 * 1024 * 1024;

/// Hangi yerel araç.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YerelArac {
    OpenCode,
    Dsh,
}

impl YerelArac {
    pub fn havuz_id(self) -> &'static str {
        match self {
            Self::OpenCode => "opencode",
            Self::Dsh => "dsh",
        }
    }

    fn etiket(self) -> &'static str {
        match self {
            Self::OpenCode => "OpenCode",
            Self::Dsh => "DeepSeek Harness",
        }
    }
}

/// Sayım sonucu (saf veri; havuza [`sayactan_havuz`] ile çevrilir).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SayimSonucu {
    pub oturum_24s: u64,
    pub oturum_7g: u64,
    pub oturum_toplam: u64,
    pub belirtec_24s: Option<u64>,
    pub belirtec_7g: Option<u64>,
    /// Son etkinlik (Unix sn).
    pub son_etkinlik_sn: Option<i64>,
    /// Kullanıcıya gösterilecek kısa not (ör. belirteç neden yok).
    pub not: Option<String>,
}

/// Okuma hatası: `Yok` kurulu değil (havuz gizlenir), `Okunamadi` geçici/biçim hatası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SayimHatasi {
    Yok,
    Okunamadi(&'static str),
}

fn ev_dizini() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

fn ortam_yolu(ad: &str) -> Option<PathBuf> {
    std::env::var_os(ad)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// OpenCode veri klasörü: ayar → `XDG_DATA_HOME\opencode` → `~\.local\share\opencode`.
pub fn opencode_dizini_coz(ayar_dizini: Option<&str>) -> Option<PathBuf> {
    if let Some(d) = ayar_dizini.map(str::trim).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    if let Some(xdg) = ortam_yolu("XDG_DATA_HOME") {
        return Some(xdg.join("opencode"));
    }
    ev_dizini().map(|ev| ev.join(".local").join("share").join("opencode"))
}

/// dsh ana klasörü: ayar → `DSH_HOME` → `~\.dsh`.
pub fn dsh_dizini_coz(ayar_dizini: Option<&str>) -> Option<PathBuf> {
    if let Some(d) = ayar_dizini.map(str::trim).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    ortam_yolu("DSH_HOME").or_else(|| ev_dizini().map(|ev| ev.join(".dsh")))
}

/// Aracın bu makinede kayıt bıraktığı (kurulu ve en az bir kez çalışmış) görünüp görünmediği.
/// Yalnız dosya/klasör varlığına bakılır, içerik okunmaz.
pub fn yerel_arac_var_mi(arac: YerelArac, ayar_dizini: Option<&str>) -> bool {
    match arac {
        YerelArac::OpenCode => opencode_dizini_coz(ayar_dizini).is_some_and(|d| {
            d.join("opencode.db").is_file() || d.join("storage").join("session").is_dir()
        }),
        YerelArac::Dsh => dsh_dizini_coz(ayar_dizini).is_some_and(|d| d.join("sessions").is_dir()),
    }
}

fn unix_sn(zaman: SystemTime) -> Option<i64> {
    zaman
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}

fn dosya_zamani_sn(yol: &Path) -> Option<i64> {
    std::fs::metadata(yol)
        .and_then(|m| m.modified())
        .ok()
        .and_then(unix_sn)
}

/// Zaman damgasını saniyeye çevirir: OpenCode `Date.now()` (ms) yazar; saniye yazan eski kayıt da
/// kabul edilir.
fn sn_normalize(deger: i64) -> i64 {
    if deger > 100_000_000_000 {
        deger / 1000
    } else {
        deger
    }
}

// ---------------------------------------------------------------------------
// OpenCode
// ---------------------------------------------------------------------------

/// OpenCode kullanımını sayar. Önce `opencode.db` (güncel sürümler), yoksa eski `storage/` JSON
/// düzeni okunur.
pub fn opencode_say(dizin: &Path, simdi_sn: i64) -> Result<SayimSonucu, SayimHatasi> {
    let db = dizin.join("opencode.db");
    if db.is_file() {
        return opencode_db_say(&db, simdi_sn);
    }
    let depo = dizin.join("storage");
    if depo.join("session").is_dir() {
        return opencode_json_say(&depo, simdi_sn);
    }
    Err(SayimHatasi::Yok)
}

fn tablo_sutunlari(b: &Connection, tablo: &str) -> Vec<String> {
    let Ok(mut s) = b.prepare(&format!("PRAGMA table_info(\"{tablo}\")")) else {
        return Vec::new();
    };
    s.query_map([], |r| r.get::<_, String>(1))
        .map(|it| it.flatten().collect())
        .unwrap_or_default()
}

fn opencode_db_say(db: &Path, simdi_sn: i64) -> Result<SayimSonucu, SayimHatasi> {
    let b = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| SayimHatasi::Okunamadi("veritabani_acilamadi"))?;
    let _ = b.busy_timeout(Duration::from_millis(250));
    let _ = b.pragma_update(None, "query_only", true);

    // Güncel geliştirme sürümü `session_v2`, yayımlanan sürümler `session` tablosunu kullanır.
    let (tablo, sutunlar) = ["session_v2", "session"]
        .iter()
        .map(|t| (*t, tablo_sutunlari(&b, t)))
        .find(|(_, s)| !s.is_empty())
        .ok_or(SayimHatasi::Okunamadi("oturum_tablosu_yok"))?;
    let var = |ad: &str| sutunlar.iter().any(|s| s == ad);
    if !var("time_updated") && !var("time_created") {
        return Err(SayimHatasi::Okunamadi("zaman_sutunu_yok"));
    }
    let zaman_sutunu = if var("time_updated") {
        "time_updated"
    } else {
        "time_created"
    };
    // ms ya da sn yazılmış olabilir; karşılaştırma saniyeye çevrilmiş değerle yapılır.
    let zaman_ifadesi = format!(
        "(CASE WHEN {zaman_sutunu} > 100000000000 THEN {zaman_sutunu} / 1000 ELSE {zaman_sutunu} END)"
    );
    let sinir_24 = simdi_sn - GUN_SN;
    let sinir_7 = simdi_sn - 7 * GUN_SN;

    let (toplam, s24, s7, son): (i64, i64, i64, Option<i64>) = b
        .query_row(
            &format!(
                "SELECT COUNT(*), \
                 COALESCE(SUM(CASE WHEN {z} >= ?1 THEN 1 ELSE 0 END), 0), \
                 COALESCE(SUM(CASE WHEN {z} >= ?2 THEN 1 ELSE 0 END), 0), \
                 MAX({z}) FROM \"{tablo}\"",
                z = zaman_ifadesi
            ),
            params![sinir_24, sinir_7],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|_| SayimHatasi::Okunamadi("oturum_sayilamadi"))?;

    let mut sonuc = SayimSonucu {
        oturum_24s: toplam_u64(s24),
        oturum_7g: toplam_u64(s7),
        oturum_toplam: toplam_u64(toplam),
        son_etkinlik_sn: son,
        ..SayimSonucu::default()
    };

    // Belirteç önce ileti zamanından sayılır: `message.data.tokens` her yanıtın kendi belirtecidir,
    // pencere sınırı ileti başına uygulanır. Oturum tablosundaki `tokens_*` sütunları oturumun ömür
    // boyu birikmiş toplamıdır; eski bir oturuma bugün tek ileti atılması tüm geçmişini "son 24 sa"
    // içine taşırdı. Bu yüzden oturum toplamı yalnız ileti tablosu yoksa ve yalnız pencere içinde
    // başlamış oturumlar için kullanılır.
    let belirtec_sutunlari: Vec<&str> = ["tokens_input", "tokens_output", "tokens_reasoning"]
        .into_iter()
        .filter(|s| var(s))
        .collect();
    let ileti = tablo_sutunlari(&b, "message");
    if ileti.iter().any(|s| s == "data") && ileti.iter().any(|s| s == "time_created") {
        let mut b24 = 0u64;
        let mut b7 = 0u64;
        let mut s = b
            .prepare(
                "SELECT time_created, data FROM message WHERE \
                 (CASE WHEN time_created > 100000000000 THEN time_created / 1000 ELSE time_created END) >= ?1",
            )
            .map_err(|_| SayimHatasi::Okunamadi("ileti_okunamadi"))?;
        let satirlar = s
            .query_map(params![sinir_7], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|_| SayimHatasi::Okunamadi("ileti_okunamadi"))?;
        for (zaman, veri) in satirlar.flatten() {
            let adet = ileti_belirteci(&veri);
            b7 += adet;
            if sn_normalize(zaman) >= sinir_24 {
                b24 += adet;
            }
        }
        // İleti verisi belirteç taşımıyorsa (şema farkı) ve oturum sütunları varsa onlara düşülür.
        if b7 > 0 || belirtec_sutunlari.is_empty() {
            sonuc.belirtec_24s = Some(b24);
            sonuc.belirtec_7g = Some(b7);
            return Ok(sonuc);
        }
    }

    if !belirtec_sutunlari.is_empty() && var("time_created") {
        // Oturum başına birikmiş sayaç (önbellek hariç): yalnız pencere içinde başlamış oturumların
        // toplamı pencereye aittir. Daha önce başlayıp pencerede güncellenen oturumun hangi kısmının
        // pencereye düştüğü bilinemez; sayılmaz ve not düşülür (değer şişmez, eksik kalabilir).
        let toplam_ifade = belirtec_sutunlari.join(" + ");
        let baslangic = "(CASE WHEN time_created > 100000000000 THEN time_created / 1000 ELSE time_created END)";
        let (b24, b7, eksik): (i64, i64, i64) = b
            .query_row(
                &format!(
                    "SELECT COALESCE(SUM(CASE WHEN {c} >= ?1 THEN {t} ELSE 0 END), 0), \
                     COALESCE(SUM(CASE WHEN {c} >= ?2 THEN {t} ELSE 0 END), 0), \
                     COALESCE(SUM(CASE WHEN {z} >= ?2 AND {c} < ?2 THEN 1 \
                     WHEN {z} >= ?1 AND {c} < ?1 THEN 1 ELSE 0 END), 0) \
                     FROM \"{tablo}\"",
                    c = baslangic,
                    z = zaman_ifadesi,
                    t = toplam_ifade
                ),
                params![sinir_24, sinir_7],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| SayimHatasi::Okunamadi("belirtec_sayilamadi"))?;
        sonuc.belirtec_24s = Some(toplam_u64(b24));
        sonuc.belirtec_7g = Some(toplam_u64(b7));
        if eksik > 0 {
            sonuc.not = Some(
                "pencereden önce başlayan oturumların belirteci sayılmaz (eksik olabilir)"
                    .to_string(),
            );
        }
        return Ok(sonuc);
    }

    sonuc.not = Some("bu OpenCode sürümünde pencere içi belirteç sayısı yok".to_string());
    Ok(sonuc)
}

fn toplam_u64(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

/// OpenCode ileti JSON'undaki `tokens.{input,output,reasoning}` toplamı (önbellek hariç).
pub fn ileti_belirteci(json_metni: &str) -> u64 {
    let Ok(v) = serde_json::from_str::<Value>(json_metni) else {
        return 0;
    };
    let Some(t) = v.get("tokens") else {
        return 0;
    };
    ["input", "output", "reasoning"]
        .iter()
        .filter_map(|k| t.get(*k).and_then(Value::as_u64))
        .sum()
}

/// Eski JSON depolaması: `storage/session/<proje>/ses_*.json` oturum, `storage/message/<oturum>/*.json`
/// ileti. Zaman olarak dosya değişim zamanı kullanılır.
fn opencode_json_say(depo: &Path, simdi_sn: i64) -> Result<SayimSonucu, SayimHatasi> {
    let sinir_24 = simdi_sn - GUN_SN;
    let sinir_7 = simdi_sn - 7 * GUN_SN;
    let mut sonuc = SayimSonucu::default();

    let oturum_kok = depo.join("session");
    let projeler =
        std::fs::read_dir(&oturum_kok).map_err(|_| SayimHatasi::Okunamadi("klasor_okunamadi"))?;
    for proje in projeler.flatten() {
        let Ok(dosyalar) = std::fs::read_dir(proje.path()) else {
            continue;
        };
        for d in dosyalar.flatten() {
            let yol = d.path();
            if yol.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            sonuc.oturum_toplam += 1;
            if let Some(z) = dosya_zamani_sn(&yol) {
                if z >= sinir_24 {
                    sonuc.oturum_24s += 1;
                }
                if z >= sinir_7 {
                    sonuc.oturum_7g += 1;
                }
                sonuc.son_etkinlik_sn = Some(sonuc.son_etkinlik_sn.map_or(z, |s| s.max(z)));
            }
        }
    }

    let ileti_kok = depo.join("message");
    let mut b24 = 0u64;
    let mut b7 = 0u64;
    let mut taranan = 0usize;
    if let Ok(oturumlar) = std::fs::read_dir(&ileti_kok) {
        'dis: for o in oturumlar.flatten() {
            // Klasör 7 günden eskiyse içine bakılmaz.
            if dosya_zamani_sn(&o.path()).is_some_and(|z| z < sinir_7) {
                continue;
            }
            let Ok(iletiler) = std::fs::read_dir(o.path()) else {
                continue;
            };
            for i in iletiler.flatten() {
                taranan += 1;
                if taranan > MAKS_ILETI_DOSYASI {
                    sonuc.not =
                        Some("ileti klasörü çok büyük; belirteç kısmen sayıldı".to_string());
                    break 'dis;
                }
                let yol = i.path();
                let Some(z) = dosya_zamani_sn(&yol) else {
                    continue;
                };
                if z < sinir_7 {
                    continue;
                }
                let Ok(metin) = std::fs::read_to_string(&yol) else {
                    continue;
                };
                let adet = ileti_belirteci(&metin);
                b7 += adet;
                if z >= sinir_24 {
                    b24 += adet;
                }
            }
        }
    }
    sonuc.belirtec_24s = Some(b24);
    sonuc.belirtec_7g = Some(b7);
    Ok(sonuc)
}

// ---------------------------------------------------------------------------
// DeepSeek Harness (dsh)
// ---------------------------------------------------------------------------

/// Oturum klasöründeki en yüksek kuşak günlüğü: `session.jsonl`, `session.vN.jsonl`, `.zstd` ekli
/// biçimler. Dönüş: (yol, sıkıştırılmış mı).
fn dsh_gunlugu_bul(oturum_dizini: &Path) -> Option<(PathBuf, bool)> {
    let mut en_iyi: Option<(u64, PathBuf, bool)> = None;
    for g in std::fs::read_dir(oturum_dizini).ok()?.flatten() {
        let ad = g.file_name().to_string_lossy().into_owned();
        let (govde, sikistirilmis) = match ad.strip_suffix(".zstd") {
            Some(g) => (g.to_string(), true),
            None => (ad.clone(), false),
        };
        let Some(ic) = govde
            .strip_prefix("session")
            .and_then(|g| g.strip_suffix(".jsonl"))
        else {
            continue;
        };
        let kusak = if ic.is_empty() {
            0
        } else if let Some(n) = ic.strip_prefix(".v").and_then(|n| n.parse::<u64>().ok()) {
            n
        } else {
            continue;
        };
        if en_iyi.as_ref().is_none_or(|(k, _, _)| kusak > *k) {
            en_iyi = Some((kusak, g.path(), sikistirilmis));
        }
    }
    en_iyi.map(|(_, y, s)| (y, s))
}

/// dsh `sessions/<proje>/<oturum>/session*.jsonl[.zstd]` düzenini sayar. Belirteç yalnız
/// sıkıştırılmamış günlükte, pencere içindeki olayların `data.usage` alanından toplanır
/// (akış parçası `*/chunk` olayları çift sayılmasın diye atlanır).
pub fn dsh_say(dizin: &Path, simdi_sn: i64) -> Result<SayimSonucu, SayimHatasi> {
    let kok = dizin.join("sessions");
    let projeler = std::fs::read_dir(&kok).map_err(|_| SayimHatasi::Yok)?;
    let sinir_24 = simdi_sn - GUN_SN;
    let sinir_7 = simdi_sn - 7 * GUN_SN;
    let sinir_24_ms = sinir_24 * 1000;
    let sinir_7_ms = sinir_7 * 1000;
    let mut sonuc = SayimSonucu::default();
    let mut b24 = 0u64;
    let mut b7 = 0u64;
    let mut sikistirilmis_var = false;
    let mut ham_var = false;
    let mut kirpilan_var = false;
    let mut taranan = 0usize;

    'dis: for proje in projeler.flatten() {
        if !proje.path().is_dir() {
            continue;
        }
        let Ok(oturumlar) = std::fs::read_dir(proje.path()) else {
            continue;
        };
        for o in oturumlar.flatten() {
            let Some((gunluk, sikistirilmis)) = dsh_gunlugu_bul(&o.path()) else {
                continue;
            };
            taranan += 1;
            if taranan > MAKS_DSH_OTURUM {
                break 'dis;
            }
            sonuc.oturum_toplam += 1;
            let Some(z) = dosya_zamani_sn(&gunluk) else {
                continue;
            };
            sonuc.son_etkinlik_sn = Some(sonuc.son_etkinlik_sn.map_or(z, |s| s.max(z)));
            if z >= sinir_24 {
                sonuc.oturum_24s += 1;
            }
            if z < sinir_7 {
                continue;
            }
            sonuc.oturum_7g += 1;
            if sikistirilmis {
                sikistirilmis_var = true;
                continue;
            }
            ham_var = true;
            let (a24, a7, kirpildi) = dsh_gunluk_belirteci(&gunluk, sinir_24_ms, sinir_7_ms);
            b24 += a24;
            b7 += a7;
            kirpilan_var |= kirpildi;
        }
    }
    if ham_var || !sikistirilmis_var {
        sonuc.belirtec_24s = Some(b24);
        sonuc.belirtec_7g = Some(b7);
    }
    let mut notlar: Vec<&str> = Vec::new();
    if sikistirilmis_var {
        notlar.push(if ham_var {
            "sıkıştırılmış dsh günlüklerinde belirteç sayılmaz (kısmi)"
        } else {
            "dsh günlükleri sıkıştırılmış; yalnız oturum sayılır"
        });
    }
    if kirpilan_var {
        notlar.push("çok büyük dsh günlüğünün yalnız son 64 MiB'ı sayıldı (kısmi)");
    }
    if !notlar.is_empty() {
        sonuc.not = Some(notlar.join("; "));
    }
    Ok(sonuc)
}

/// Tek ham dsh günlüğünden pencere içi belirteç: `(son 24 sa, son 7 gün, kırpıldı mı)`. Olay
/// zamanı `time` (ms) alanından okunur; zamanı olmayan satır dosya penceresine dahil sayılmaz.
fn dsh_gunluk_belirteci(yol: &Path, sinir_24_ms: i64, sinir_7_ms: i64) -> (u64, u64, bool) {
    dsh_gunluk_belirteci_sinirli(yol, sinir_24_ms, sinir_7_ms, MAKS_GUNLUK_BAYT)
}

/// [`dsh_gunluk_belirteci`] gövdesi; `azami_bayt` testte küçültülebilsin diye ayrı.
///
/// Olaylar günlüğün sonuna eklenir: dosya `azami_bayt`'tan büyükse baştan değil **sondan**
/// `azami_bayt` okunur (en yeni olaylar kaybolmasın) ve sonuç kırpılmış diye işaretlenir. Göreli
/// `time` için gereken başlık (`createdAt`) her durumda ilk satırdan ayrıca okunur.
fn dsh_gunluk_belirteci_sinirli(
    yol: &Path,
    sinir_24_ms: i64,
    sinir_7_ms: i64,
    azami_bayt: u64,
) -> (u64, u64, bool) {
    use std::io::{Read, Seek, SeekFrom};

    let Ok(mut dosya) = std::fs::File::open(yol) else {
        return (0, 0, false);
    };
    let boyut = dosya.metadata().map(|m| m.len()).unwrap_or(0);

    // Başlıktaki `createdAt` (ms) olay `time` alanının göreli olup olmadığını anlamaya yarar.
    let mut taban_ms = 0i64;
    {
        let mut ilk_satir = Vec::new();
        let mut baslik = BufReader::new((&dosya).take(MAKS_SATIR_BAYT as u64));
        if baslik.read_until(b'\n', &mut ilk_satir).is_ok()
            && let Ok(v) = serde_json::from_slice::<Value>(&ilk_satir)
            && v.get("type").and_then(Value::as_str) == Some("session")
        {
            taban_ms = v.get("createdAt").and_then(Value::as_i64).unwrap_or(0);
        }
    }

    let kirpildi = boyut > azami_bayt;
    // Kırpılınca bir bayt geriden başlanır ve ilk (yarım ya da önceki) satır atılır.
    let baslangic = if kirpildi { boyut - azami_bayt - 1 } else { 0 };
    if dosya.seek(SeekFrom::Start(baslangic)).is_err() {
        return (0, 0, kirpildi);
    }
    let mut okuyucu = BufReader::new(dosya.take(azami_bayt + 1));
    if kirpildi {
        let mut atilan = Vec::new();
        if okuyucu.read_until(b'\n', &mut atilan).is_err() {
            return (0, 0, kirpildi);
        }
    }

    let mut b24 = 0u64;
    let mut b7 = 0u64;
    for satir in okuyucu.split(b'\n') {
        let Ok(satir) = satir else {
            break;
        };
        if satir.len() > MAKS_SATIR_BAYT || satir.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&satir) else {
            continue;
        };
        let tur = v.get("type").and_then(Value::as_str).unwrap_or("");
        if tur == "session" || tur.ends_with("/chunk") {
            continue;
        }
        let Some(kullanim) = v.get("data").and_then(|d| d.get("usage")) else {
            continue;
        };
        let adet: u64 = ["inputTokens", "outputTokens", "reasoningTokens"]
            .iter()
            .filter_map(|k| kullanim.get(*k).and_then(Value::as_u64))
            .sum();
        let Some(zaman) = v.get("time").and_then(Value::as_i64) else {
            continue;
        };
        // `time` mutlak (ms) ya da oturum başına göre görelidir.
        let mutlak = if zaman < 100_000_000_000 {
            taban_ms + zaman
        } else {
            zaman
        };
        if mutlak >= sinir_7_ms {
            b7 += adet;
            if mutlak >= sinir_24_ms {
                b24 += adet;
            }
        }
    }
    (b24, b7, kirpildi)
}

// ---------------------------------------------------------------------------
// Havuz üretimi ve bağlayıcı
// ---------------------------------------------------------------------------

/// Sayımı havuza çevirir (pencere/yüzde yok; `ek.kullanim` dolu).
pub fn sayactan_havuz(arac: YerelArac, sayim: &SayimSonucu, simdi: UtcZaman) -> HavuzAnlik {
    let son_etkinlik = sayim
        .son_etkinlik_sn
        .and_then(UtcZaman::from_unix_timestamp);
    let mut havuz = bos_havuz(arac, Durum::Ok);
    havuz.kaynak = Some(KaynakBilgisi {
        tur: KaynakTuru::YerelGunluk,
        guven: GuvenEtiketi::YerelKayit,
        okuma: simdi,
        yas_sn: 0,
        hassasiyet: None,
    });
    havuz.ek = Some(EkBilgi {
        kullanim: Some(KullanimSayaci {
            kota_yok: true,
            oturum_24s: sayim.oturum_24s,
            oturum_7g: sayim.oturum_7g,
            oturum_toplam: sayim.oturum_toplam,
            belirtec_24s: sayim.belirtec_24s,
            belirtec_7g: sayim.belirtec_7g,
            son_etkinlik,
        }),
        ..EkBilgi::default()
    });
    havuz.notlar.push("kota yok, kullanım sayacı".to_string());
    if let Some(n) = &sayim.not {
        havuz.notlar.push(n.clone());
    }
    havuz.hesapla_etkin_durum();
    havuz
}

fn bos_havuz(arac: YerelArac, durum: Durum) -> HavuzAnlik {
    HavuzAnlik {
        platform: arac.havuz_id().to_string(),
        etiket: arac.etiket().to_string(),
        durum,
        etkin_kalan: None,
        baglayici_pencere: None,
        kullanilabilir: None,
        kaynak: None,
        pencereler: BTreeMap::new(),
        alt_sayaclar: BTreeMap::new(),
        tahmin: BTreeMap::new(),
        baski: None,
        bakiye: None,
        ek: None,
        hata: None,
        notlar: Vec::new(),
        kesif: None,
        etkin_kalan_zincir: None,
    }
}

/// Saf okuma: klasörü çözer, sayar ve havuz üretir (testlerde doğrudan çağrılır).
pub fn yerel_sayac_oku(arac: YerelArac, dizin: Option<&Path>, simdi: UtcZaman) -> HavuzAnlik {
    let Some(dizin) = dizin else {
        let mut h = bos_havuz(arac, Durum::Baglanmadi);
        h.notlar.push("kullanıcı klasörü bulunamadı".to_string());
        return h;
    };
    let simdi_sn = simdi.as_offset().unix_timestamp();
    let sayim = match arac {
        YerelArac::OpenCode => opencode_say(dizin, simdi_sn),
        YerelArac::Dsh => dsh_say(dizin, simdi_sn),
    };
    match sayim {
        Ok(s) => sayactan_havuz(arac, &s, simdi),
        Err(SayimHatasi::Yok) => {
            let mut h = bos_havuz(arac, Durum::Baglanmadi);
            h.notlar.push(format!("{} kurulu değil", arac.etiket()));
            h
        }
        Err(SayimHatasi::Okunamadi(tur)) => {
            let mut h = bos_havuz(arac, Durum::Hata);
            h.hata = Some(HataBilgisi {
                tur: tur.to_string(),
                http_kodu: None,
            });
            h
        }
    }
}

/// OpenCode / dsh yerel sayaç bağlayıcısı. Ayarda kapalıysa `Kapali` döner (ana ekranda görünmez).
pub struct YerelSayacBaglayicisi {
    arac: YerelArac,
    ayar: YerelSayacAyari,
}

impl YerelSayacBaglayicisi {
    pub fn yeni(arac: YerelArac, ayar: YerelSayacAyari) -> Self {
        Self { arac, ayar }
    }

    fn dizin(&self) -> Option<PathBuf> {
        match self.arac {
            YerelArac::OpenCode => opencode_dizini_coz(self.ayar.dizin.as_deref()),
            YerelArac::Dsh => dsh_dizini_coz(self.ayar.dizin.as_deref()),
        }
    }
}

impl Baglayici for YerelSayacBaglayicisi {
    fn havuz_id(&self) -> &str {
        self.arac.havuz_id()
    }

    fn oku<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = BaglayiciSonucu> + Send + 'a>> {
        Box::pin(async move {
            let arac = self.arac;
            let havuz = if !self.ayar.etkin {
                let mut h = bos_havuz(arac, Durum::Kapali);
                h.notlar.push("ayardan kapalı".to_string());
                h
            } else {
                let dizin = self.dizin();
                // Dosya taraması bloklayıcıdır; çalışma zamanı iş parçacığını tutmasın.
                tokio::task::spawn_blocking(move || yerel_sayac_oku(arac, dizin.as_deref(), simdi))
                    .await
                    .unwrap_or_else(|_| {
                        let mut h = bos_havuz(arac, Durum::Hata);
                        h.hata = Some(HataBilgisi {
                            tur: "okunamadi".to_string(),
                            http_kodu: None,
                        });
                        h
                    })
            };
            BaglayiciSonucu {
                havuz_id: arac.havuz_id().to_string(),
                gecici_hata: havuz.durum == Durum::Hata,
                havuz,
                retry_after_sn: None,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ileti_belirteci_onbellegi_saymaz_bozuk_json_sifir() {
        assert_eq!(
            ileti_belirteci(
                r#"{"role":"assistant","tokens":{"input":10,"output":5,"reasoning":2,"cache":{"read":1000,"write":3}}}"#
            ),
            17
        );
        assert_eq!(ileti_belirteci(r#"{"role":"user"}"#), 0);
        assert_eq!(ileti_belirteci("bozuk"), 0);
    }

    #[test]
    fn sn_normalize_ms_ve_sn_kabul_eder() {
        assert_eq!(sn_normalize(1_763_300_000_000), 1_763_300_000);
        assert_eq!(sn_normalize(1_763_300_000), 1_763_300_000);
    }

    #[test]
    fn buyuk_dsh_gunlugu_sondan_okunur_en_yeni_olay_kaybolmaz() {
        let d = std::env::temp_dir().join(format!("cetvel-dsh-kirp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let yol = d.join("session.jsonl");
        let taban = 1_763_000_000_000i64;
        let mut metin = format!("{{\"type\":\"session\",\"createdAt\":{taban}}}\n");
        // Eski olaylar (pencere dışı) baştaki dolguyu oluşturur.
        for i in 0..200 {
            metin.push_str(&format!(
                "{{\"type\":\"assistant/message\",\"time\":{},\"data\":{{\"usage\":{{\"inputTokens\":1}}}}}}\n",
                i
            ));
        }
        // En yeni olay göreli zamanla, dosyanın sonunda.
        metin.push_str(
            "{\"type\":\"assistant/message\",\"time\":86400000,\"data\":{\"usage\":{\"inputTokens\":500,\"outputTokens\":7}}}\n",
        );
        std::fs::write(&yol, &metin).unwrap();
        let s24 = taban + 86_400_000 - 1000;
        let s7 = s24;
        // Sınır dosyadan küçük: kırpılır ama son olay (göreli zaman + başlık tabanı) sayılır.
        let (b24, b7, kirpildi) = dsh_gunluk_belirteci_sinirli(&yol, s24, s7, 512);
        assert!(kirpildi);
        assert_eq!((b24, b7), (507, 507));
        // Sınır dosyadan büyükse kırpma yok, sonuç aynı.
        let (b24, _, kirpildi) = dsh_gunluk_belirteci_sinirli(&yol, s24, s7, 1 << 20);
        assert!(!kirpildi);
        assert_eq!(b24, 507);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dizin_ayardan_once_gelir() {
        assert_eq!(
            opencode_dizini_coz(Some(r"D2:\x\oc")),
            Some(PathBuf::from(r"D2:\x\oc"))
        );
        assert_eq!(
            dsh_dizini_coz(Some(" /tmp/dsh ")),
            Some(PathBuf::from("/tmp/dsh"))
        );
    }
}
