//! Antigravity (`agy`) `conversation_summaries.db` okuyucusu.
//!
//! Veritabanı salt okunur açılır (`SQLITE_OPEN_READ_ONLY`, `query_only`, kısa `busy_timeout`);
//! WAL kipinde okuyucu yazanı engellemez. Yalnız durum/ebeveyn/sayı sütunları seçilir: `title`,
//! `preview`, `workspace_uris`, `raw_summary` ve FTS tabloları hiç sorgulanmaz.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use cetvel_model::UtcZaman;
use rusqlite::types::Value as SqlDeger;
use rusqlite::{Connection, OpenFlags};

use crate::kayit::{AjanKaydi, Kurallar};
use crate::model::{AjanDurumu, Platform, kisa_ad};
use crate::{KaynakDurumu, OkumaSonucu};

/// Değişim imzası: veritabanı ve `-wal` dosyasının boyut + değişim zamanı.
type Imza = [(u64, Option<SystemTime>); 2];

#[derive(Debug)]
struct DbOnbellek {
    imza: Imza,
    okundu: UtcZaman,
    kayitlar: Vec<AjanKaydi>,
}

/// Bir ya da birden çok özet veritabanını yoklayan okuyucu.
#[derive(Debug)]
pub struct AntigravityOkuyucu {
    db_yollari: Vec<PathBuf>,
    onbellek: BTreeMap<PathBuf, DbOnbellek>,
}

/// Değişiklik olmasa da veritabanı en geç bu aralıkla yeniden sorgulanır.
const ZORUNLU_YENILEME_SN: i64 = 30;

impl AntigravityOkuyucu {
    pub fn yeni(db_yollari: Vec<PathBuf>) -> Self {
        Self {
            db_yollari,
            onbellek: BTreeMap::new(),
        }
    }

    pub fn tara(&mut self, simdi: UtcZaman, kurallar: &Kurallar) -> OkumaSonucu {
        let mut bulunan = 0usize;
        let mut basarili = 0usize;
        let mut kayitlar = Vec::new();
        for yol in self.db_yollari.clone() {
            if !yol.is_file() {
                self.onbellek.remove(&yol);
                continue;
            }
            bulunan += 1;
            let imza = imza_al(&yol);
            let taze = self
                .onbellek
                .get(&yol)
                .is_some_and(|o| o.imza == imza && simdi.fark_sn(o.okundu) < ZORUNLU_YENILEME_SN);
            if !taze {
                match ozet_db_oku(&yol, simdi, kurallar.pencere_sn) {
                    Ok(k) => {
                        self.onbellek.insert(
                            yol.clone(),
                            DbOnbellek {
                                imza,
                                okundu: simdi,
                                kayitlar: k,
                            },
                        );
                    }
                    Err(_) => {
                        // Geçici kilit/bozulma: önceki sonuç (varsa) korunur.
                        if let Some(o) = self.onbellek.get(&yol) {
                            kayitlar.extend(o.kayitlar.iter().cloned());
                        }
                        continue;
                    }
                }
            }
            if let Some(o) = self.onbellek.get(&yol) {
                basarili += 1;
                kayitlar.extend(o.kayitlar.iter().cloned());
            }
        }
        let durum = if bulunan == 0 {
            KaynakDurumu::Bulunamadi
        } else if basarili == 0 {
            KaynakDurumu::Okunamadi
        } else {
            KaynakDurumu::Izleniyor
        };
        OkumaSonucu { durum, kayitlar }
    }
}

fn imza_al(yol: &Path) -> Imza {
    let tek = |p: &Path| {
        std::fs::metadata(p)
            .map(|m| (m.len(), m.modified().ok()))
            .unwrap_or((0, None))
    };
    let mut wal = yol.as_os_str().to_owned();
    wal.push("-wal");
    [tek(yol), tek(Path::new(&wal))]
}

/// Antigravity zaman metnini (`2026-11-16 05:42:10.1234567+00:00`, RFC3339 ya da Go'nun
/// `… +0000 UTC` biçimi) UTC'ye çevirir.
pub fn agy_zamani(ham: &str) -> Option<UtcZaman> {
    let mut s = ham.trim().to_string();
    if s.is_empty() {
        return None;
    }
    if let Some(bas) = s.strip_suffix(" UTC") {
        s = bas.to_string();
    }
    if s.len() > 10 && s.as_bytes()[10] == b' ' {
        s.replace_range(10..11, "T");
    }
    // ` +0000` → `+00:00`
    if let Some(i) = s.rfind([' ']) {
        let ek = s[i + 1..].to_string();
        if ek.len() == 5
            && (ek.starts_with('+') || ek.starts_with('-'))
            && let (Some(saat), Some(dakika)) = (ek.get(..3), ek.get(3..))
        {
            s = format!("{}{saat}:{dakika}", &s[..i]);
        }
    }
    // Saniye kesri 9 haneden uzunsa kırp.
    if let Some(nokta) = s.find('.') {
        let son = s[nokta + 1..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|k| nokta + 1 + k)
            .unwrap_or(s.len());
        if son - nokta - 1 > 9 {
            s.replace_range(nokta + 10..son, "");
        }
    }
    UtcZaman::parse_tarih_veya_zaman(&s).ok()
}

fn metin_degeri(v: &SqlDeger) -> Option<String> {
    match v {
        SqlDeger::Text(t) => Some(t.clone()),
        SqlDeger::Integer(i) => Some(i.to_string()),
        SqlDeger::Real(r) => Some(r.to_string()),
        _ => None,
    }
}

fn sayi_degeri(v: &SqlDeger) -> Option<i64> {
    match v {
        SqlDeger::Integer(i) => Some(*i),
        SqlDeger::Real(r) => Some(*r as i64),
        SqlDeger::Text(t) => match t.trim().to_ascii_lowercase().as_str() {
            "true" => Some(1),
            "false" => Some(0),
            s => s.parse().ok(),
        },
        _ => None,
    }
}

const SUTUNLAR: [&str; 10] = [
    "conversation_id",
    "parent_conversation_id",
    "nesting_depth",
    "agent_name",
    "status",
    "not_fully_idle",
    "killed",
    "step_count",
    "last_modified_time",
    "last_user_input_time",
];

/// Özet veritabanını salt okunur sorgular; son `pencere_sn` içinde değişen konuşmaları döndürür.
pub fn ozet_db_oku(
    yol: &Path,
    simdi: UtcZaman,
    pencere_sn: i64,
) -> rusqlite::Result<Vec<AjanKaydi>> {
    let baglanti = Connection::open_with_flags(
        yol,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    baglanti.busy_timeout(Duration::from_millis(250))?;
    baglanti.pragma_update(None, "query_only", true)?;

    let mevcut: Vec<String> = {
        let mut s = baglanti.prepare("PRAGMA table_info(conversation_summaries)")?;
        s.query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<_, _>>()?
    };
    if !mevcut.iter().any(|c| c == "conversation_id")
        || !mevcut.iter().any(|c| c == "last_modified_time")
    {
        return Ok(Vec::new());
    }
    let secim = SUTUNLAR
        .iter()
        .map(|c| {
            if mevcut.iter().any(|m| m == c) {
                format!("`{c}`")
            } else {
                format!("NULL AS `{c}`")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sorgu = format!(
        "SELECT {secim} FROM conversation_summaries ORDER BY last_modified_time DESC LIMIT 500"
    );

    let ebeveynler: HashMap<String, String> = {
        let var: bool = baglanti
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='conversation_parents'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)?;
        if var {
            let mut s =
                baglanti.prepare("SELECT conversation_id, parent_id FROM conversation_parents")?;
            s.query_map([], |r| {
                Ok((r.get::<_, SqlDeger>(0)?, r.get::<_, SqlDeger>(1)?))
            })?
            .filter_map(Result::ok)
            .filter_map(|(a, b)| Some((metin_degeri(&a)?, metin_degeri(&b)?)))
            .collect()
        } else {
            HashMap::new()
        }
    };

    let mut s = baglanti.prepare(&sorgu)?;
    let satirlar = s.query_map([], |r| {
        let mut d = Vec::with_capacity(SUTUNLAR.len());
        for i in 0..SUTUNLAR.len() {
            d.push(r.get::<_, SqlDeger>(i)?);
        }
        Ok(d)
    })?;

    let mut sonuc = Vec::new();
    for satir in satirlar.flatten() {
        let Some(kimlik) = metin_degeri(&satir[0]).filter(|k| !k.is_empty()) else {
            continue;
        };
        let Some(son) = metin_degeri(&satir[8]).and_then(|t| agy_zamani(&t)) else {
            continue;
        };
        if simdi.fark_sn(son) > pencere_sn {
            continue;
        }
        let ebeveyn = metin_degeri(&satir[1])
            .filter(|e| !e.is_empty())
            .or_else(|| ebeveynler.get(&kimlik).cloned())
            .filter(|e| !e.is_empty() && *e != kimlik);
        let derinlik = sayi_degeri(&satir[2])
            .map(|d| d.clamp(0, 64) as u32)
            .filter(|d| *d > 0)
            .unwrap_or(u32::from(ebeveyn.is_some()));
        let durum_metni = metin_degeri(&satir[4])
            .unwrap_or_default()
            .to_ascii_uppercase();
        let tam_bosta_degil = sayi_degeri(&satir[5]).is_some_and(|v| v != 0);
        let olduruldu = sayi_degeri(&satir[6]).is_some_and(|v| v != 0);

        let mut a = AjanKaydi::yeni(Platform::Antigravity, kimlik, son);
        a.ebeveyn_yerel = ebeveyn;
        a.derinlik = derinlik;
        a.ad = metin_degeri(&satir[3])
            .and_then(|n| kisa_ad(&n))
            .unwrap_or_else(|| "Antigravity".to_string());
        a.baslama = metin_degeri(&satir[9])
            .and_then(|t| agy_zamani(&t))
            .filter(|b| *b <= son)
            .or(Some(son));
        a.sayilar.adim = sayi_degeri(&satir[7]).and_then(|n| u64::try_from(n).ok());
        a.durum = agy_durumu(&durum_metni, tam_bosta_degil, olduruldu);
        if a.durum.bitmis_mi() {
            a.bitis = Some(son);
            if a.durum == AjanDurumu::Hata {
                a.hata_turu = Some(crate::siniflandir::hata_turu_metinden(&durum_metni));
            }
        }
        sonuc.push(a);
    }
    Ok(sonuc)
}

/// Özet satırındaki durum alanlarını normalize duruma çevirir.
pub fn agy_durumu(durum: &str, tam_bosta_degil: bool, olduruldu: bool) -> AjanDurumu {
    let d = durum.to_ascii_uppercase();
    let var = |k: &[&str]| k.iter().any(|x| d.contains(x));
    if olduruldu {
        AjanDurumu::Iptal
    } else if var(&["ERROR", "FAIL"]) {
        AjanDurumu::Hata
    } else if var(&["CANCEL", "ABORT", "KILL"]) {
        AjanDurumu::Iptal
    } else if var(&["WAIT", "PAUSE", "BLOCK", "PENDING", "APPROVAL"]) {
        AjanDurumu::Bekliyor
    } else if tam_bosta_degil || var(&["RUNNING", "ACTIVE", "BUSY", "STREAM"]) {
        AjanDurumu::Calisiyor
    } else {
        AjanDurumu::Bitti
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agy_zaman_bicimleri_cozulur() {
        let beklenen = UtcZaman::parse_tarih_veya_zaman("2026-11-16T05:42:10.1234567Z").unwrap();
        assert_eq!(
            agy_zamani("2026-11-16 05:42:10.1234567+00:00"),
            Some(beklenen)
        );
        assert_eq!(
            agy_zamani("2026-11-16 08:42:10.1234567 +0300 +03"),
            None,
            "tanınmayan Go biçimi sessizce reddedilir"
        );
        assert_eq!(
            agy_zamani("2026-11-16 05:42:10.1234567 +0000 UTC"),
            Some(beklenen)
        );
        assert!(agy_zamani("").is_none());
        // Bozuk, ASCII olmayan ek bayt sınırını ortadan bölse de panik yok.
        assert!(agy_zamani("2026-11-16 05:42:10 +é00").is_none());
        assert!(agy_zamani("2026-11-16 05:42:10 -0é0").is_none());
        assert!(agy_zamani("ğğğğğğğğğğğ +0000").is_none());
    }

    #[test]
    fn durumlar_esle() {
        assert_eq!(
            agy_durumu("CASCADE_RUN_STATUS_IDLE", false, false),
            AjanDurumu::Bitti
        );
        assert_eq!(
            agy_durumu("CASCADE_RUN_STATUS_IDLE", true, false),
            AjanDurumu::Calisiyor
        );
        assert_eq!(
            agy_durumu("CASCADE_RUN_STATUS_RUNNING", false, false),
            AjanDurumu::Calisiyor
        );
        assert_eq!(agy_durumu("", false, true), AjanDurumu::Iptal);
        assert_eq!(
            agy_durumu("CASCADE_RUN_STATUS_ERROR", false, false),
            AjanDurumu::Hata
        );
        assert_eq!(
            agy_durumu("CASCADE_RUN_STATUS_WAITING_FOR_USER", false, false),
            AjanDurumu::Bekliyor
        );
    }
}
