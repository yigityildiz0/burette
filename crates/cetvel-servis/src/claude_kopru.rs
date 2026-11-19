use std::collections::BTreeMap;

use cetvel_model::{
    AltSayac, BakiyeBilgisi, Durum, EkBilgi, GuvenEtiketi, HavuzAnlik, KaynakBilgisi, KaynakTuru,
    PencereOlcum, UtcZaman,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// İki Claude sohbetinin aynı 2 dakika içinde köprü sorgusu tekrarlamasını engelleyen varsayılan kira süresi.
pub const VARSAYILAN_KIRA_SURE_SN: u64 = 120;
const EN_FAZLA_KIRA_SURE_SN: u64 = 600;
const OAUTH_ONCELIK_ESIGI_SN: i64 = 120;
const OKUMA_GELECEK_TOLERANSI_SN: i64 = 60;
const OKUMA_EN_ESKI_SN: i64 = 3600;

#[derive(Debug, Error)]
pub enum ClaudeKopruHatasi {
    #[error("Geçersiz yüzde değeri: {0}")]
    GecersizYuzde(String),
    #[error("Geçersiz sıfırlama zamanı: {0}")]
    GecersizZaman(String),
    #[error("Geçerli pencere verisi bulunamadı")]
    EksikPencere,
    #[error("Desteklenmeyen Claude köprü yükü: {0}")]
    GecersizBicim(String),
}

/// İstemcinin `POST /v1/claude/kira` (veya `/v1/kopru/claude/kira`) isteği.
#[derive(Debug, Clone, Deserialize)]
pub struct KiraIstegi {
    pub istemci: String,
    #[serde(default)]
    pub sure_sn: Option<u64>,
}

/// Kira talebine dönen yanıt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KiraSonucu {
    pub verildi: bool,
    pub sahip: String,
    pub bitis: UtcZaman,
    pub kalan_sn: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neden: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AktifKira {
    pub istemci: String,
    pub baslangic: UtcZaman,
    pub bitis: UtcZaman,
}

/// İki Claude sohbetinin aynı pencerede arka arkaya `oauth/usage` çağırmasını önleyen kira yöneticisi.
#[derive(Debug, Clone)]
pub struct KiraYoneticisi {
    varsayilan_sure_sn: u64,
    aktif: Option<AktifKira>,
}

impl Default for KiraYoneticisi {
    fn default() -> Self {
        Self::yeni(VARSAYILAN_KIRA_SURE_SN)
    }
}

impl KiraYoneticisi {
    pub fn yeni(varsayilan_sure_sn: u64) -> Self {
        Self {
            varsayilan_sure_sn: varsayilan_sure_sn.clamp(10, EN_FAZLA_KIRA_SURE_SN),
            aktif: None,
        }
    }

    pub fn talep_et(
        &mut self,
        istemci: &str,
        istenen_sure_sn: Option<u64>,
        simdi: UtcZaman,
    ) -> KiraSonucu {
        let temiz_istemci = istemci.trim();
        if temiz_istemci.is_empty() {
            return KiraSonucu {
                verildi: false,
                sahip: String::new(),
                bitis: simdi,
                kalan_sn: 0,
                neden: Some("istemci adı boş olamaz".to_string()),
            };
        }

        if let Some(mevcut) = &self.aktif {
            let kalan = mevcut.bitis.fark_sn(simdi);
            if kalan > 0 && mevcut.istemci != temiz_istemci {
                return KiraSonucu {
                    verildi: false,
                    sahip: mevcut.istemci.clone(),
                    bitis: mevcut.bitis,
                    kalan_sn: kalan as u64,
                    neden: Some("başka bir sohbet aktif kiraya sahip".to_string()),
                };
            }
        }

        let sure = istenen_sure_sn
            .unwrap_or(self.varsayilan_sure_sn)
            .clamp(5, EN_FAZLA_KIRA_SURE_SN);
        let bitis = simdi.ekle_sn(sure as i64);
        self.aktif = Some(AktifKira {
            istemci: temiz_istemci.to_string(),
            baslangic: simdi,
            bitis,
        });

        KiraSonucu {
            verildi: true,
            sahip: temiz_istemci.to_string(),
            bitis,
            kalan_sn: sure,
            neden: None,
        }
    }

    pub fn aktif_kira(&self, simdi: UtcZaman) -> Option<&AktifKira> {
        self.aktif.as_ref().filter(|k| k.bitis.fark_sn(simdi) > 0)
    }
}

struct AyristirilmisClaude {
    havuz_id: String,
    kaynak_turu: KaynakTuru,
    hassasiyet: f64,
    okuma: UtcZaman,
    pencereler: BTreeMap<String, PencereOlcum>,
    alt_sayaclar: BTreeMap<String, AltSayac>,
    bakiye: Option<BakiyeBilgisi>,
    plan_turu: Option<String>,
    olagan_kullanim: Option<bool>,
    /// Bozuk olduğu için atlanan pencerelerin notları (diğer pencereler yine uygulanır).
    atlanan: Vec<String>,
}

/// Köprü yükündeki havuz kimliği kalıbı: `claude_[a-z0-9_]{1,32}` (yoklanan havuz adlarıyla çakışmaz).
pub fn kopru_havuz_kimligi_gecerli_mi(id: &str) -> bool {
    id.strip_prefix("claude_").is_some_and(|govde| {
        (1..=32).contains(&govde.len())
            && govde
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
}

/// Yükte gelmeyen pencereler/alt sayaçlar ve bakiye, sıfırlaması geçmediyse önceki havuzdan taşınır
/// (ör. statusLine yalnız `five_hour` gönderdi; haftalık %0 kaybolup 5s %70 "rahat" göstermesin).
fn eksik_alanlari_tasi(havuz: &mut HavuzAnlik, eski: &HavuzAnlik, simdi: UtcZaman) {
    let gecerli = |s: Option<UtcZaman>| s.is_none_or(|z| simdi.fark_sn(z) < 0);
    for (ad, olcum) in &eski.pencereler {
        if !havuz.pencereler.contains_key(ad) && gecerli(olcum.sifirlama) {
            havuz.pencereler.insert(ad.clone(), olcum.clone());
            havuz
                .notlar
                .push(format!("{ad}: bu yükte yok, önceki okumadan taşındı"));
        }
    }
    for (ad, sayac) in &eski.alt_sayaclar {
        if !havuz.alt_sayaclar.contains_key(ad) && gecerli(sayac.sifirlama) {
            havuz.alt_sayaclar.insert(ad.clone(), sayac.clone());
        }
    }
    if havuz.bakiye.is_none() && eski.bakiye.is_some() {
        havuz.bakiye = eski.bakiye.clone();
        if let (Some(ek), Some(eski_ek)) = (havuz.ek.as_mut(), eski.ek.as_ref()) {
            ek.kredi_bakiye = ek
                .kredi_bakiye
                .take()
                .or_else(|| eski_ek.kredi_bakiye.clone());
        }
    }
}

/// `POST /v1/claude` (ve `/v1/kopru/claude`) ile gelen veriyi doğrular ve havuzu günceller.
///
/// Dönüşteki `bool` değer verinin gerçekten uygulandığını (`true`), yoksa daha yüksek
/// hassasiyetli taze bir `oauth_usage` kaydı korunarak atlandığını (`false`) belirtir.
pub fn claude_havuzu_guncelle(
    mevcut: Option<&HavuzAnlik>,
    ham: Value,
    simdi: UtcZaman,
) -> Result<(HavuzAnlik, bool), ClaudeKopruHatasi> {
    let yeni = claude_yuku_ayristir(ham, simdi)?;

    if let Some(eski) = mevcut
        && let Some(eski_kaynak) = &eski.kaynak
    {
        let eski_hassasiyet = eski_kaynak.hassasiyet.unwrap_or(1.0);
        let yeni_daha_kaba = yeni.hassasiyet > eski_hassasiyet + 0.2;
        let taze = simdi.fark_sn(eski_kaynak.okuma) <= OAUTH_ONCELIK_ESIGI_SN;
        let tukendi = yeni
            .pencereler
            .values()
            .any(|p| p.kalan <= 0.0 || p.kullanilan >= 1.0);
        let sifirlandi = yeni.pencereler.iter().any(|(ad, yp)| {
            eski.pencereler
                .get(ad)
                .and_then(|ep| ep.sifirlama)
                .zip(yp.sifirlama)
                .is_some_and(|(es, ys)| (ys.fark_sn(es)).abs() > 60)
        });

        if yeni_daha_kaba && taze && !tukendi && !sifirlandi {
            return Ok((eski.clone(), false));
        }
    }

    let mut notlar = Vec::new();
    if let Some(eski) = mevcut {
        for (pencere_ad, yeni_olcum) in &yeni.pencereler {
            if let Some(eski_olcum) = eski.pencereler.get(pencere_ad)
                && let (Some(es), Some(ys)) = (eski_olcum.sifirlama, yeni_olcum.sifirlama)
                && (ys.fark_sn(es)).abs() <= 60
                && yeni_olcum.kullanilan + 1e-6 < eski_olcum.kullanilan
            {
                notlar.push(format!("{pencere_ad}: geri gitti"));
            }
        }
    }

    let mut ek = EkBilgi {
        olagan_kullanim: yeni.olagan_kullanim,
        plan_turu: yeni.plan_turu,
        ..EkBilgi::default()
    };
    if let Some(bakiye) = &yeni.bakiye {
        ek.kredi_bakiye = Some(format!("{:.2} {}", bakiye.toplam, bakiye.para_birimi));
    }

    let mut havuz = HavuzAnlik {
        platform: "claude".to_string(),
        // Kullanıcıya abonelik adı gösterilmez: "claude_max" havuzunun etiketi yalnız "Claude".
        etiket: if yeni.havuz_id == "claude_max" {
            "Claude".to_string()
        } else {
            format!("Claude ({})", yeni.havuz_id)
        },
        durum: Durum::Ok,
        etkin_kalan: None,
        baglayici_pencere: None,
        kullanilabilir: None,
        kaynak: Some(KaynakBilgisi {
            tur: yeni.kaynak_turu,
            guven: GuvenEtiketi::ResmiArayuz,
            okuma: yeni.okuma,
            yas_sn: simdi.fark_sn(yeni.okuma).max(0) as u64,
            hassasiyet: Some(yeni.hassasiyet),
        }),
        pencereler: yeni.pencereler,
        alt_sayaclar: yeni.alt_sayaclar,
        tahmin: BTreeMap::new(),
        baski: None,
        bakiye: yeni.bakiye,
        ek: Some(ek),
        hata: None,
        notlar,
    };
    havuz.notlar.extend(yeni.atlanan);
    if let Some(eski) = mevcut {
        eksik_alanlari_tasi(&mut havuz, eski, simdi);
    }

    havuz.hesapla_etkin_durum();
    Ok((havuz, true))
}

/// Sıfırlama saati geçmiş pencereleri sıfırlanmış (`kullanilan: 0.0`, `Hesap`) kabul eder.
pub fn suresi_dolan_pencereleri_hesapla(havuz: &mut HavuzAnlik, simdi: UtcZaman) -> bool {
    let mut degisti = false;
    for (pencere_ad, olcum) in &mut havuz.pencereler {
        if let Some(sifirlama) = olcum.sifirlama
            && simdi.fark_sn(sifirlama) >= 0
        {
            olcum.kullanilan = 0.0;
            olcum.kalan = 1.0;
            olcum.sifirlama = None;
            degisti = true;
            let not = format!("{pencere_ad}: sıfırlama saati geçti, hesaplandı");
            if !havuz.notlar.contains(&not) {
                havuz.notlar.push(not);
            }
        }
    }
    if degisti {
        if let Some(kaynak) = &mut havuz.kaynak {
            kaynak.tur = KaynakTuru::Hesap;
            kaynak.guven = GuvenEtiketi::Tahmini;
            kaynak.yas_sn = 0;
        }
        havuz.hesapla_etkin_durum();
    }
    degisti
}

fn claude_yuku_ayristir(
    ham: Value,
    simdi: UtcZaman,
) -> Result<AyristirilmisClaude, ClaudeKopruHatasi> {
    let obj = ham
        .as_object()
        .ok_or_else(|| ClaudeKopruHatasi::GecersizBicim("JSON nesnesi bekleniyordu".to_string()))?;

    let havuz_id = obj
        .get("havuz")
        .or_else(|| obj.get("havuz_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("claude_max")
        .to_string();
    if !kopru_havuz_kimligi_gecerli_mi(&havuz_id) {
        return Err(ClaudeKopruHatasi::GecersizBicim(format!(
            "havuz kimliği claude_[a-z0-9_]{{1,32}} kalıbına uymuyor: {}",
            havuz_id.chars().take(40).collect::<String>()
        )));
    }

    let kaynak_str = obj
        .get("kaynak")
        .or_else(|| obj.get("source"))
        .and_then(Value::as_str)
        .unwrap_or("oauth_usage")
        .to_ascii_lowercase();

    let (kaynak_turu, varsayilan_hassasiyet) = if kaynak_str.contains("status") {
        (KaynakTuru::Statusline, 1.0)
    } else if kaynak_str.contains("ratelimit")
        || kaynak_str.contains("header")
        || kaynak_str.contains("yanit")
    {
        (KaynakTuru::ModKoprusu, 1.0)
    } else {
        (KaynakTuru::ModKoprusu, 0.1)
    };
    let hassasiyet = obj
        .get("hassasiyet")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(varsayilan_hassasiyet);

    let okuma = match obj.get("okuma").filter(|v| !v.is_null()) {
        None => simdi,
        Some(v) => {
            let z = zaman_coz(v).ok_or_else(|| {
                ClaudeKopruHatasi::GecersizZaman(format!("okuma ayrıştırılamadı: {v}"))
            })?;
            // Gelecekteki okuma havuzu hiç bayatlatmaz; çok eski okuma zaten bayattır.
            if z.fark_sn(simdi) > OKUMA_GELECEK_TOLERANSI_SN || simdi.fark_sn(z) > OKUMA_EN_ESKI_SN
            {
                return Err(ClaudeKopruHatasi::GecersizZaman(format!(
                    "okuma aralık dışı ([şimdi - 1sa, şimdi + 60sn]): {}",
                    z.to_iso_z()
                )));
            }
            z
        }
    };

    let mut plan_turu = obj
        .get("plan_turu")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    if plan_turu.is_none()
        && let Some(plan_val) = obj.get("plan")
    {
        plan_turu = plan_val
            .get("status")
            .or_else(|| plan_val.get("name"))
            .or_else(|| plan_val.get("type"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| plan_val.as_str().map(ToString::to_string));
    }

    let olagan_kullanim = obj.get("olagan_kullanim").and_then(Value::as_bool);

    let mut pencereler = BTreeMap::new();
    let mut alt_sayaclar = BTreeMap::new();
    // Bozuk tek pencere tüm yükü düşürmez: atlanır ve not düşülür; hiç geçerli pencere
    // kalmazsa ilk hata döner.
    let mut ilk_hata: Option<ClaudeKopruHatasi> = None;
    let mut atlanan = Vec::new();
    let mut ekle = |ad: Option<&str>,
                    val: &Value,
                    pencereler: &mut BTreeMap<String, PencereOlcum>,
                    alt_sayaclar: &mut BTreeMap<String, AltSayac>| {
        let sonuc = match ad {
            Some(ad) => pencere_veya_alt_ekle(ad, val, simdi, pencereler, alt_sayaclar),
            None => Err(ClaudeKopruHatasi::GecersizBicim(
                "pencere adı (label/kind) eksik".to_string(),
            )),
        };
        if let Err(e) = sonuc {
            atlanan.push(format!("{}: atlandı ({e})", ad.unwrap_or("?")));
            ilk_hata.get_or_insert(e);
        }
    };

    if let Some(p_obj) = obj.get("pencereler").and_then(Value::as_object) {
        for (anahtar, val) in p_obj {
            ekle(Some(anahtar), val, &mut pencereler, &mut alt_sayaclar);
        }
    }

    if let Some(win_arr) = obj.get("windows").and_then(Value::as_array) {
        for item in win_arr {
            let label = item
                .get("label")
                .or_else(|| item.get("kind"))
                .or_else(|| item.get("name"))
                .and_then(Value::as_str);
            ekle(label, item, &mut pencereler, &mut alt_sayaclar);
        }
    }

    if let Some(lim_arr) = obj.get("limits").and_then(Value::as_array) {
        for item in lim_arr {
            let kind = item
                .get("kind")
                .or_else(|| item.get("label"))
                .and_then(Value::as_str);
            ekle(kind, item, &mut pencereler, &mut alt_sayaclar);
        }
    }

    // Doğrudan Anthropic OAuth üst düzey anahtarları (`five_hour`, `seven_day`, vb.)
    for ham_ad in [
        "five_hour",
        "seven_day",
        "seven_day_sonnet",
        "seven_day_opus",
        "seven_day_cowork",
        "seven_day_oauth_apps",
        "fiveHour",
        "sevenDay",
    ] {
        if let Some(val) = obj.get(ham_ad)
            && val.is_object()
        {
            ekle(Some(ham_ad), val, &mut pencereler, &mut alt_sayaclar);
        }
    }

    if pencereler.is_empty() {
        return Err(ilk_hata.unwrap_or(ClaudeKopruHatasi::EksikPencere));
    }

    let bakiye = obj
        .get("extraUsage")
        .or_else(|| obj.get("extra_usage"))
        .and_then(ekstra_kullanim_coz);

    Ok(AyristirilmisClaude {
        havuz_id,
        kaynak_turu,
        hassasiyet,
        okuma,
        pencereler,
        alt_sayaclar,
        bakiye,
        plan_turu,
        olagan_kullanim,
        atlanan,
    })
}

fn pencere_veya_alt_ekle(
    ham_ad: &str,
    val: &Value,
    simdi: UtcZaman,
    pencereler: &mut BTreeMap<String, PencereOlcum>,
    alt_sayaclar: &mut BTreeMap<String, AltSayac>,
) -> Result<(), ClaudeKopruHatasi> {
    let kullanilan = oran_coz(val)?;
    let kalan = (1.0 - kullanilan).clamp(0.0, 1.0);
    let sifirlama = val
        .get("resetsAt")
        .or_else(|| val.get("resets_at"))
        .or_else(|| val.get("sifirlama"))
        .and_then(|v| {
            if v.is_null() {
                None
            } else {
                Some(zaman_dogrula(v, simdi))
            }
        })
        .transpose()?;

    match pencere_ad_sinifla(ham_ad) {
        ClaudePencereSinifi::AnaPencere { ad, uzunluk_dk } => {
            pencereler.insert(
                ad.to_string(),
                PencereOlcum {
                    uzunluk_dk,
                    kullanilan,
                    kalan,
                    sifirlama,
                },
            );
        }
        ClaudePencereSinifi::AltSayac(ad) => {
            alt_sayaclar.insert(
                ad,
                AltSayac {
                    kullanilan,
                    kalan: Some(kalan),
                    uzunluk_dk: Some(10080),
                    sifirlama,
                },
            );
        }
    }
    Ok(())
}

enum ClaudePencereSinifi {
    AnaPencere { ad: &'static str, uzunluk_dk: u64 },
    AltSayac(String),
}

fn pencere_ad_sinifla(ham: &str) -> ClaudePencereSinifi {
    let k = ham.trim().to_ascii_lowercase();
    if k.contains("sonnet")
        || k.contains("opus")
        || k.contains("cowork")
        || k.contains("oauth_apps")
        || k.contains("fable")
    {
        let kisa = if k.contains("sonnet") {
            "sonnet"
        } else if k.contains("opus") {
            "opus"
        } else if k.contains("cowork") {
            "cowork"
        } else if k.contains("oauth") {
            "oauth_apps"
        } else {
            "fable"
        };
        return ClaudePencereSinifi::AltSayac(kisa.to_string());
    }

    if k == "5s"
        || k == "5h"
        || k == "five_hour"
        || k == "fivehour"
        || k.contains("session")
        || k.contains("5-hour")
        || k.contains("5 hour")
        || k.contains("5 saat")
    {
        ClaudePencereSinifi::AnaPencere {
            ad: "5s",
            uzunluk_dk: 300,
        }
    } else if k == "hafta"
        || k == "7d"
        || k == "seven_day"
        || k == "sevenday"
        || k.contains("week")
        || k.contains("hafta")
        || k.contains("7-day")
    {
        ClaudePencereSinifi::AnaPencere {
            ad: "hafta",
            uzunluk_dk: 10080,
        }
    } else {
        ClaudePencereSinifi::AltSayac(k.replace(' ', "_"))
    }
}

fn oran_coz(val: &Value) -> Result<f64, ClaudeKopruHatasi> {
    if let Some(k) = val.get("kullanilan").and_then(Value::as_f64) {
        if !k.is_finite() || !(0.0..=1.0).contains(&k) {
            return Err(ClaudeKopruHatasi::GecersizYuzde(format!("{k}")));
        }
        return Ok(k);
    }

    if let Some(y) = val
        .get("used_percentage")
        .or_else(|| val.get("percentUsed"))
        .or_else(|| val.get("percent_used"))
        .or_else(|| val.get("utilization"))
        .or_else(|| val.get("yuzde"))
        .and_then(Value::as_f64)
    {
        if !y.is_finite() || !(0.0..=100.0).contains(&y) {
            return Err(ClaudeKopruHatasi::GecersizYuzde(format!("{y}%")));
        }
        return Ok((y / 100.0).clamp(0.0, 1.0));
    }

    if let Some(kalan_oran) = val.get("kalan").and_then(Value::as_f64) {
        if !kalan_oran.is_finite() || !(0.0..=1.0).contains(&kalan_oran) {
            return Err(ClaudeKopruHatasi::GecersizYuzde(format!(
                "kalan {kalan_oran}"
            )));
        }
        return Ok((1.0 - kalan_oran).clamp(0.0, 1.0));
    }

    Err(ClaudeKopruHatasi::GecersizYuzde(
        "kullanilan / used_percentage / percentUsed / utilization alanı eksik".to_string(),
    ))
}

fn zaman_dogrula(val: &Value, simdi: UtcZaman) -> Result<UtcZaman, ClaudeKopruHatasi> {
    let z = zaman_coz(val)
        .ok_or_else(|| ClaudeKopruHatasi::GecersizZaman(format!("zaman ayrıştırılamadı: {val}")))?;
    let en_eski = simdi.ekle_sn(-300);
    let en_yeni = simdi.ekle_sn(8 * 86_400);
    if z.fark_sn(en_eski) < 0 || z.fark_sn(en_yeni) > 0 {
        return Err(ClaudeKopruHatasi::GecersizZaman(format!(
            "resets_at aralık dışı ([şimdi - 5dk, şimdi + 8gün]): {}",
            z.to_iso_z()
        )));
    }
    Ok(z)
}

fn zaman_coz(val: &Value) -> Option<UtcZaman> {
    match val {
        Value::String(s) => UtcZaman::parse_tarih_veya_zaman(s).ok(),
        Value::Number(n) => {
            let ts = n.as_i64()?;
            let sn = if ts > 10_000_000_000 { ts / 1000 } else { ts };
            UtcZaman::from_unix_timestamp(sn)
        }
        _ => None,
    }
}

fn ekstra_kullanim_coz(val: &Value) -> Option<BakiyeBilgisi> {
    let etkin = val
        .get("isEnabled")
        .or_else(|| val.get("is_enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let limit = val
        .get("monthlyLimit")
        .or_else(|| val.get("monthly_limit"))
        .and_then(Value::as_f64);
    let used = val
        .get("usedCredits")
        .or_else(|| val.get("used_credits"))
        .and_then(Value::as_f64);

    if let (Some(limit), Some(used)) = (limit, used) {
        let kalan = (limit - used).max(0.0);
        return Some(BakiyeBilgisi {
            para_birimi: "USD".to_string(),
            toplam: kalan,
            hibe: None,
            yuklenen: Some(limit),
            kullanilabilir: Some(etkin && kalan > 0.0),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).unwrap()
    }

    #[test]
    fn get_usage_yukunu_cozer_ve_baglayici_pencere_secer() {
        let simdi = z("2026-11-15T18:00:00Z");
        let yuk = json!({
            "plan": { "status": "max_20x" },
            "windows": [
                { "label": "Current session", "percentUsed": 16, "resetsAt": "2026-11-15T22:00:00Z" },
                { "label": "Current week (all models)", "percentUsed": 64, "resetsAt": "2026-11-20T07:00:00Z" }
            ],
            "extraUsage": { "isEnabled": true, "monthlyLimit": 50.0, "usedCredits": 12.5 }
        });

        let (havuz, uygulandi) = claude_havuzu_guncelle(None, yuk, simdi).unwrap();
        assert!(uygulandi);
        assert_eq!(havuz.durum, Durum::Ok);
        assert_eq!(havuz.baglayici_pencere.as_deref(), Some("hafta"));
        assert!((havuz.etkin_kalan.unwrap() - 0.36).abs() < 1e-6);
        assert!((havuz.pencereler["5s"].kalan - 0.84).abs() < 1e-6);
        assert_eq!(havuz.bakiye.as_ref().unwrap().toplam, 37.5);
    }

    #[test]
    fn geri_gitti_algilanir_ve_oauth_hassasiyeti_korunur() {
        let simdi = z("2026-11-15T18:00:00Z");
        let oauth_yuk = json!({
            "kaynak": "oauth_usage",
            "five_hour": { "utilization": 41.3, "resets_at": "2026-11-15T22:00:00Z" },
            "seven_day": { "utilization": 36.2, "resets_at": "2026-11-20T07:00:00Z" }
        });
        let (ilk, u1) = claude_havuzu_guncelle(None, oauth_yuk, simdi).unwrap();
        assert!(u1);

        // 30 sn sonra kaba yanit_ratelimits gelirse ezmez
        let kaba_yuk = json!({
            "kaynak": "yanit_ratelimits",
            "limits": [
                { "kind": "5h", "percentUsed": 41, "resetsAt": "2026-11-15T22:00:00Z" },
                { "kind": "7d", "percentUsed": 36, "resetsAt": "2026-11-20T07:00:00Z" }
            ]
        });
        let (korunan, u2) =
            claude_havuzu_guncelle(Some(&ilk), kaba_yuk, simdi.ekle_sn(30)).unwrap();
        assert!(!u2);
        assert!((korunan.pencereler["5s"].kullanilan - 0.413).abs() < 1e-6);

        // Aynı dönemde düşüş gösteren yeni oauth verisi `geri gitti` notu ekler
        let dusuk_oauth = json!({
            "kaynak": "oauth_usage",
            "five_hour": { "utilization": 35.0, "resets_at": "2026-11-15T22:00:00Z" },
            "seven_day": { "utilization": 36.2, "resets_at": "2026-11-20T07:00:00Z" }
        });
        let (guncel, u3) =
            claude_havuzu_guncelle(Some(&korunan), dusuk_oauth, simdi.ekle_sn(60)).unwrap();
        assert!(u3);
        assert!(guncel.notlar.iter().any(|n| n.contains("geri gitti")));
    }

    #[test]
    fn suresi_dolan_pencere_hesaplanir_ve_gecersiz_yuzde_reddedilir() {
        let simdi = z("2026-11-15T18:00:00Z");
        let gecersiz =
            json!({ "five_hour": { "utilization": 105.0, "resets_at": "2026-11-15T20:00:00Z" } });
        assert!(claude_havuzu_guncelle(None, gecersiz, simdi).is_err());

        let eski_zaman =
            json!({ "five_hour": { "utilization": 20.0, "resets_at": "2026-11-10T20:00:00Z" } });
        assert!(claude_havuzu_guncelle(None, eski_zaman, simdi).is_err());

        let gecerli = json!({
            "five_hour": { "utilization": 85.0, "resets_at": "2026-11-15T19:00:00Z" }
        });
        let (mut havuz, _) = claude_havuzu_guncelle(None, gecerli, simdi).unwrap();
        let degisti = suresi_dolan_pencereleri_hesapla(&mut havuz, z("2026-11-15T19:05:00Z"));
        assert!(degisti);
        assert_eq!(havuz.pencereler["5s"].kalan, 1.0);
        assert_eq!(havuz.kaynak.as_ref().unwrap().tur, KaynakTuru::Hesap);
    }

    /// Claude Code statusLine stdin'inin gerçek şeması (`rate_limits.*.used_percentage` 0-100,
    /// `resets_at` epoch sn); `statusline_kota_yuku` kaynağı `statusline` olarak işaretler.
    #[test]
    fn statusline_gercek_semasi_kabul_edilir_ve_hassasiyet_bir() {
        let simdi = z("2026-11-15T18:00:00Z");
        let resets = simdi.ekle_sn(3 * 3600).as_offset().unix_timestamp();
        let haftalik = simdi.ekle_sn(4 * 86_400).as_offset().unix_timestamp();
        let stdin = json!({
            "model": { "display_name": "Opus" },
            "rate_limits": {
                "five_hour": { "used_percentage": 41, "resets_at": resets },
                "seven_day": { "used_percentage": 23.5, "resets_at": haftalik }
            }
        });
        let yuk = crate::bagla::statusline_kota_yuku(&stdin).expect("yük");
        let (havuz, uygulandi) = claude_havuzu_guncelle(None, yuk, simdi).expect("kabul");
        assert!(uygulandi);
        assert!((havuz.pencereler["5s"].kullanilan - 0.41).abs() < 1e-9);
        assert!((havuz.pencereler["hafta"].kullanilan - 0.235).abs() < 1e-9);
        let kaynak = havuz.kaynak.as_ref().unwrap();
        assert_eq!(kaynak.tur, KaynakTuru::Statusline);
        assert_eq!(kaynak.hassasiyet, Some(1.0));
    }

    #[test]
    fn bozuk_tek_pencere_digerini_dusurmez() {
        let simdi = z("2026-11-15T18:00:00Z");
        let yuk = json!({
            "kaynak": "statusline",
            "five_hour": { "used_percentage": 30, "resets_at": simdi.ekle_sn(3600).to_iso_z() },
            "seven_day": { "resets_at": simdi.ekle_sn(86_400).to_iso_z() }
        });
        let (havuz, _) = claude_havuzu_guncelle(None, yuk, simdi).expect("5s uygulanır");
        assert!(havuz.pencereler.contains_key("5s"));
        assert!(!havuz.pencereler.contains_key("hafta"));
        assert!(havuz.notlar.iter().any(|n| n.contains("atlandı")));
    }

    #[test]
    fn kismi_yuk_eski_haftalik_pencereyi_korur() {
        let simdi = z("2026-11-15T18:00:00Z");
        let tam = json!({
            "five_hour": { "utilization": 30, "resets_at": simdi.ekle_sn(3600).to_iso_z() },
            "seven_day": { "utilization": 100, "resets_at": simdi.ekle_sn(3 * 86_400).to_iso_z() },
            "seven_day_opus": { "utilization": 50, "resets_at": simdi.ekle_sn(3 * 86_400).to_iso_z() },
            "extraUsage": { "isEnabled": true, "monthlyLimit": 50.0, "usedCredits": 10.0 }
        });
        let (ilk, _) = claude_havuzu_guncelle(None, tam, simdi).unwrap();
        assert_eq!(ilk.kullanilabilir, Some(false));

        let kismi = json!({
            "five_hour": { "utilization": 35, "resets_at": simdi.ekle_sn(3600).to_iso_z() },
            "seven_day": null
        });
        let (yeni, uygulandi) =
            claude_havuzu_guncelle(Some(&ilk), kismi, simdi.ekle_sn(300)).unwrap();
        assert!(uygulandi);
        assert_eq!(yeni.pencereler["hafta"].kalan, 0.0, "haftalık %0 korunur");
        assert_eq!(yeni.baglayici_pencere.as_deref(), Some("hafta"));
        assert_eq!(yeni.etkin_kalan, Some(0.0));
        assert_eq!(yeni.kullanilabilir, Some(false));
        assert!(yeni.alt_sayaclar.contains_key("opus"));
        assert!(yeni.bakiye.is_some());

        // Haftalık sıfırlama geçtiyse eski pencere taşınmaz.
        let sonra = simdi.ekle_sn(3 * 86_400 + 60);
        let yuk = json!({
            "five_hour": { "utilization": 5, "resets_at": sonra.ekle_sn(3600).to_iso_z() }
        });
        let (gec, _) = claude_havuzu_guncelle(Some(&ilk), yuk, sonra).unwrap();
        assert!(!gec.pencereler.contains_key("hafta"));
    }

    #[test]
    fn okuma_ve_havuz_kimligi_dogrulanir() {
        let simdi = z("2026-11-15T18:00:00Z");
        let pencere = json!({ "utilization": 10, "resets_at": simdi.ekle_sn(3600).to_iso_z() });
        let gelecek = json!({ "okuma": "2099-01-01T00:00:00Z", "five_hour": pencere.clone() });
        assert!(claude_havuzu_guncelle(None, gelecek, simdi).is_err());
        let eski =
            json!({ "okuma": simdi.ekle_sn(-7200).to_iso_z(), "five_hour": pencere.clone() });
        assert!(claude_havuzu_guncelle(None, eski, simdi).is_err());
        let yakin = json!({ "okuma": simdi.ekle_sn(30).to_iso_z(), "five_hour": pencere.clone() });
        assert!(claude_havuzu_guncelle(None, yakin, simdi).is_ok());

        let sahte = json!({ "havuz": "codex_plus", "five_hour": pencere.clone() });
        assert!(claude_havuzu_guncelle(None, sahte, simdi).is_err());
        let uzun =
            json!({ "havuz": format!("claude_{}", "a".repeat(40)), "five_hour": pencere.clone() });
        assert!(claude_havuzu_guncelle(None, uzun, simdi).is_err());
        let iyi = json!({ "havuz": "claude_pro", "five_hour": pencere });
        assert!(claude_havuzu_guncelle(None, iyi, simdi).is_ok());
    }

    #[test]
    fn kira_yoneticisi_iki_dakikada_ikinci_sohbeti_engeller() {
        let simdi = z("2026-11-15T18:00:00Z");
        let mut kira = KiraYoneticisi::default();

        let ilk = kira.talep_et("sohbet-1", None, simdi);
        assert!(ilk.verildi);
        assert_eq!(ilk.kalan_sn, 120);

        let ikinci = kira.talep_et("sohbet-2", None, simdi.ekle_sn(45));
        assert!(!ikinci.verildi);
        assert_eq!(ikinci.sahip, "sohbet-1");
        assert_eq!(ikinci.kalan_sn, 75);

        // Aynı sohbet yenileyebilir
        let yenile = kira.talep_et("sohbet-1", None, simdi.ekle_sn(50));
        assert!(yenile.verildi);

        // Kira bitince sohbet-2 alabilir
        let sonraki = kira.talep_et("sohbet-2", None, simdi.ekle_sn(175));
        assert!(sonraki.verildi);
        assert_eq!(sonraki.sahip, "sohbet-2");
    }
}
