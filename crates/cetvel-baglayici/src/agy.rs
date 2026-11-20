use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use cetvel_depo::{AgyAyari, AgyOkumaIzni};
use cetvel_model::{
    Durum, GuvenEtiketi, HataBilgisi, HavuzAnlik, KaynakBilgisi, KaynakTuru, PencereOlcum, UtcZaman,
};
use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;

use crate::job_object::JobSuresKorumasi;
use crate::{Baglayici, BaglayiciSonucu};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const AGY_HAVUZLARI: [&str; 3] = ["agy_gemini", "agy_claude_gpt", "agy_argon"];

/// Antigravity (`agy -p /usage`) kota bağlayıcısı.
/// Varsayılan olarak kapalıdır; `istek_uzerine` veya `otomatik` izin kapısıyla çalışır.
/// `~/.gemini/antigravity/daemon/hub_*.json` dosyalarına kesinlikle erişmez.
pub struct AntigravityBaglayicisi {
    ayar: AgyAyari,
    saatlik_cagrilar: Mutex<Vec<UtcZaman>>,
}

impl AntigravityBaglayicisi {
    pub fn yeni(ayar: AgyAyari) -> Self {
        Self {
            ayar,
            saatlik_cagrilar: Mutex::new(Vec::new()),
        }
    }

    pub fn izin(&self) -> AgyOkumaIzni {
        self.ayar.okuma
    }

    /// Ayar yeniden yüklenince saatlik çağrı tavanı sıfırlanmasın diye eski bağlayıcının
    /// sayacını devralır (kota harcayan `agy` çalıştırması sınırsız tekrarlanmaz).
    pub fn sayaci_devral(&self, eski: &AntigravityBaglayicisi) {
        if std::ptr::eq(self, eski) {
            return;
        }
        let kopya = eski
            .saatlik_cagrilar
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        if let Ok(mut g) = self.saatlik_cagrilar.lock() {
            *g = kopya;
        }
    }

    /// İzin kapısını ve saatlik tavanı kontrol ederek `agy -p /usage` okuması yapar.
    pub async fn istekle_veya_zamanli_oku(
        &self,
        simdi: UtcZaman,
        istek_uzerine_mi: bool,
    ) -> Vec<BaglayiciSonucu> {
        match self.ayar.okuma {
            AgyOkumaIzni::Kapali => {
                return kapali_sonuclar(
                    "agy okuması varsayılan olarak kapalı (ayarlar.toml: [agy].okuma)",
                );
            }
            AgyOkumaIzni::IstekUzerine if !istek_uzerine_mi => {
                return kapali_sonuclar("agy yalnızca istek üzerine okunacak şekilde ayarlı");
            }
            _ => {}
        }

        if !self.saatlik_kotayi_ayir(simdi) {
            return hata_sonuclari(
                "saatlik_sinir",
                None,
                "agy saatlik okuma sınırı aşıldı",
                false,
            );
        }

        let Some(ikili) = self.ikili_yolu_coz() else {
            return baglanmadi_sonuclari("agy CLI kurulumu bulunamadı");
        };

        let asim = Duration::from_secs(self.ayar.zaman_asimi_sn.max(5));
        match agy_surec_oku(&ikili, asim).await {
            Ok(cikti) => agy_cikti_ayristir(&cikti, simdi)
                .into_iter()
                .map(|(havuz_id, havuz)| {
                    let gecici_hata = havuz.durum == Durum::Hata;
                    BaglayiciSonucu {
                        havuz_id,
                        havuz,
                        retry_after_sn: None,
                        gecici_hata,
                    }
                })
                .collect(),
            Err(err_tur) => hata_sonuclari(err_tur, None, "agy /usage okunamadı", true),
        }
    }

    fn ikili_yolu_coz(&self) -> Option<PathBuf> {
        if let Some(ozel) = self.ayar.ikili_yol.as_deref() {
            let p = PathBuf::from(ozel.trim());
            if p.is_file() {
                return Some(p);
            }
        }
        agy_ikili_coz()
    }

    fn saatlik_kotayi_ayir(&self, simdi: UtcZaman) -> bool {
        let tavan = self.ayar.saatlik_tavan.max(1) as usize;
        let bir_saat_once = simdi.ekle_sn(-3600);
        let Ok(mut guard) = self.saatlik_cagrilar.lock() else {
            return false;
        };
        guard.retain(|t| *t > bir_saat_once);
        if guard.len() >= tavan {
            return false;
        }
        guard.push(simdi);
        true
    }
}

impl Baglayici for AntigravityBaglayicisi {
    fn havuz_id(&self) -> &str {
        "agy_gemini"
    }

    fn havuz_kimlikleri(&self) -> Vec<String> {
        AGY_HAVUZLARI.iter().map(|s| (*s).to_string()).collect()
    }

    fn oku<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = BaglayiciSonucu> + Send + 'a>> {
        Box::pin(async move {
            let mut liste = self.oku_coklu(simdi).await;
            if liste.is_empty() {
                hata_sonucu(
                    "agy_gemini",
                    "Gemini",
                    "bos_sonuc",
                    None,
                    "agy boş sonuç döndürdü",
                    true,
                )
            } else {
                liste.remove(0)
            }
        })
    }

    fn oku_coklu<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = Vec<BaglayiciSonucu>> + Send + 'a>> {
        Box::pin(async move { self.istekle_veya_zamanli_oku(simdi, false).await })
    }
}

/// `PATH` ve bilinen kullanıcı dışı kurulum yollarından `agy` ikilisini çözer (`~/.gemini` okunmaz).
pub fn agy_ikili_coz() -> Option<PathBuf> {
    let aday_adlar = ["agy.exe", "agy.cmd", "agy.bat", "agy"];
    if let Some(path_var) = std::env::var_os("PATH") {
        for dizin in std::env::split_paths(&path_var) {
            for ad in aday_adlar {
                let aday = dizin.join(ad);
                if aday.is_file() {
                    return Some(aday);
                }
            }
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let npm_dizin = PathBuf::from(appdata).join("npm");
        for ad in ["agy.cmd", "agy.exe"] {
            let aday = npm_dizin.join(ad);
            if aday.is_file() {
                return Some(aday);
            }
        }
    }
    None
}

/// `agy -p /usage --output-format json --print-timeout 40s` komutunu MSYS olmadan doğrudan çalıştırır.
pub async fn agy_surec_oku(
    ikili_yol: &Path,
    zaman_asimi: Duration,
) -> Result<String, &'static str> {
    let cmd_uzantisi = ikili_yol
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));

    let mut cmd = if cmd_uzantisi {
        let mut c = Command::new("cmd.exe");
        c.arg("/C").arg(ikili_yol);
        c
    } else {
        Command::new(ikili_yol)
    };

    cmd.args([
        "-p",
        "/usage",
        "--output-format",
        "json",
        "--print-timeout",
        "40s",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn().map_err(|_| "surec_baslatilamadi")?;
    let job = JobSuresKorumasi::yeni().ok();
    if let (Some(j), Some(h)) = (&job, child.raw_handle()) {
        let _ = j.sureci_ekle(h);
    }

    let cikti_sonucu = tokio::time::timeout(zaman_asimi, async {
        let mut stdout = child.stdout.take().ok_or("stdout_yok")?;
        let mut buf = Vec::new();
        use tokio::io::AsyncReadExt;
        stdout
            .read_to_end(&mut buf)
            .await
            .map_err(|_| "okuma_hatasi")?;
        let durum = child.wait().await.map_err(|_| "bekleme_hatasi")?;
        if !durum.success() {
            return Err("cikis_hatasi");
        }
        String::from_utf8(buf).map_err(|_| "utf8_hatasi")
    })
    .await;

    if let Some(j) = job {
        j.sonlandir();
        drop(j);
    }
    let _ = child.kill().await;

    match cikti_sonucu {
        Ok(ic) => ic,
        Err(_) => Err("zaman_asimi"),
    }
}

#[derive(Debug, Deserialize)]
struct HamAgyRoot {
    command: Option<HamAgyCommand>,
    data: Option<HamAgyData>,
    #[serde(default)]
    groups: Vec<HamAgyGroup>,
}

#[derive(Debug, Deserialize)]
struct HamAgyCommand {
    data: Option<HamAgyData>,
}

#[derive(Debug, Deserialize)]
struct HamAgyData {
    #[serde(default)]
    groups: Vec<HamAgyGroup>,
}

#[derive(Debug, Deserialize)]
struct HamAgyGroup {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    buckets: Vec<HamAgyBucket>,
}

#[derive(Debug, Deserialize)]
struct HamAgyBucket {
    id: Option<String>,
    window: Option<String>,
    remaining_fraction: Option<f64>,
    used_fraction: Option<f64>,
    remaining_percent: Option<f64>,
    reset_time: Option<Value>,
}

/// `agy -p /usage` JSON çıktısını (veya yedek metin tablosunu) 3 Cetvel havuzuna ayrıştırır.
pub fn agy_cikti_ayristir(cikti: &str, simdi: UtcZaman) -> BTreeMap<String, HavuzAnlik> {
    if let Some(havuzlar) = agy_json_ayristir(cikti, simdi) {
        return havuzlar;
    }
    if let Some(havuzlar) = agy_tablo_ayristir(cikti, simdi) {
        return havuzlar;
    }

    let mut hata_map = BTreeMap::new();
    hata_map.insert(
        "agy_gemini".to_string(),
        agy_hata_havuzu(
            "Antigravity Gemini",
            Durum::Hata,
            "sema_degisti",
            None,
            vec!["agy /usage çıktısı ayrıştırılamadı".into()],
        ),
    );
    hata_map
}

fn agy_json_ayristir(cikti: &str, simdi: UtcZaman) -> Option<BTreeMap<String, HavuzAnlik>> {
    let json_deger = serde_json::from_str::<Value>(cikti.trim())
        .ok()
        .or_else(|| {
            cikti
                .lines()
                .rev()
                .find_map(|s| serde_json::from_str::<Value>(s.trim()).ok())
        })?;

    let root: HamAgyRoot = serde_json::from_value(json_deger).ok()?;
    let mut gruplar = Vec::new();
    if let Some(cmd) = root.command
        && let Some(d) = cmd.data
    {
        gruplar.extend(d.groups);
    }
    if let Some(d) = root.data {
        gruplar.extend(d.groups);
    }
    gruplar.extend(root.groups);

    if gruplar.is_empty() {
        return None;
    }

    let mut pencere_haritasi: BTreeMap<&'static str, BTreeMap<String, PencereOlcum>> =
        BTreeMap::new();

    for grup in gruplar {
        let grup_id = grup.id.as_deref().unwrap_or_default();
        for kova in grup.buckets {
            let kova_id = kova.id.as_deref().unwrap_or(grup_id);
            let Some((havuz_id, pencere_ad, uzunluk_dk)) =
                kova_esle(kova_id, kova.window.as_deref())
            else {
                continue;
            };
            let Some(kalan) = kova_kalan_orani(&kova) else {
                continue;
            };
            let sifirlama = kova.reset_time.as_ref().and_then(zaman_degeri_coz);
            let olcum = PencereOlcum {
                uzunluk_dk,
                kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                kalan,
                sifirlama,
            };

            let havuz_pencereleri = pencere_haritasi.entry(havuz_id).or_default();
            match havuz_pencereleri.get_mut(&pencere_ad) {
                Some(mevcut) => {
                    // Oynaklık nedeniyle aynı pencere için en kısıtlayıcı (en düşük kalan) değer alınır.
                    if olcum.kalan < mevcut.kalan {
                        *mevcut = olcum;
                    }
                }
                None => {
                    havuz_pencereleri.insert(pencere_ad, olcum);
                }
            }
        }
    }

    if pencere_haritasi.is_empty() {
        return None;
    }

    Some(havuzlari_insa_et(pencere_haritasi, simdi))
}

fn kova_kalan_orani(kova: &HamAgyBucket) -> Option<f64> {
    if let Some(rem) = kova.remaining_fraction
        && rem.is_finite()
    {
        return Some(rem.clamp(0.0, 1.0));
    }
    if let Some(used) = kova.used_fraction
        && used.is_finite()
    {
        return Some((1.0 - used).clamp(0.0, 1.0));
    }
    if let Some(pct) = kova.remaining_percent
        && pct.is_finite()
    {
        return Some((pct / 100.0).clamp(0.0, 1.0));
    }
    None
}

fn bes_saatlik_baslik_mi(alt: &str) -> bool {
    alt.contains("5s")
        || alt.contains("5h")
        || alt.contains("5 saat")
        || alt.contains("five hour")
        || alt.contains("five-hour")
}

fn haftalik_baslik_mi(alt: &str) -> bool {
    alt.contains("hafta") || alt.contains("week") || alt.contains("7d")
}

fn satirdan_sifirlama_cikar(satir: &str) -> Option<UtcZaman> {
    satir
        .split(|c: char| c.is_whitespace() || c == '|')
        .find_map(|tok| UtcZaman::parse_tarih_veya_zaman(tok).ok())
}

fn kova_esle(id: &str, window: Option<&str>) -> Option<(&'static str, String, u64)> {
    let alt_id = id.to_ascii_lowercase();
    let alt_win = window.unwrap_or_default().to_ascii_lowercase();

    let (pencere_ad, uzunluk_dk) =
        if bes_saatlik_baslik_mi(&alt_win) || bes_saatlik_baslik_mi(&alt_id) {
            ("5s".to_string(), 300)
        } else {
            ("hafta".to_string(), 10080)
        };

    if alt_id.contains("argon") {
        Some(("agy_argon", "hafta".to_string(), 10080))
    } else if alt_id.contains("3p") || alt_id.contains("claude") || alt_id.contains("gpt") {
        Some(("agy_claude_gpt", pencere_ad, uzunluk_dk))
    } else if alt_id.contains("gemini") {
        Some(("agy_gemini", pencere_ad, uzunluk_dk))
    } else {
        None
    }
}

/// JSON yerine düz tablo metni (`Gemini ... 5s ...`, `Claude-GPT ...`, `Argon ...`) gelirse ayrıştırır.
fn agy_tablo_ayristir(cikti: &str, simdi: UtcZaman) -> Option<BTreeMap<String, HavuzAnlik>> {
    let mut pencere_haritasi: BTreeMap<&'static str, BTreeMap<String, PencereOlcum>> =
        BTreeMap::new();
    let mut sutun_5s: Option<usize> = None;
    let mut sutun_hafta: Option<usize> = None;
    let mut tablo_used_mi = false;

    for satir in cikti.lines() {
        let alt = satir.to_ascii_lowercase();
        let hucreler: Vec<&str> = if satir.contains('|') {
            satir
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect()
        } else {
            Vec::new()
        };

        let havuz_id = if alt.contains("argon") {
            "agy_argon"
        } else if alt.contains("claude") || alt.contains("gpt") || alt.contains("3p") {
            "agy_claude_gpt"
        } else if alt.contains("gemini") {
            "agy_gemini"
        } else {
            // Yalnız harf içeren başlık satırı sütunları günceller; Markdown ayırıcı satırı
            // (`|---|:--:|`) başlığın `used` bayrağını ve sütun konumlarını silmesin.
            let baslik_satiri = satir.chars().any(char::is_alphabetic);
            if hucreler.len() >= 2 && baslik_satiri {
                sutun_5s = hucreler
                    .iter()
                    .position(|h| bes_saatlik_baslik_mi(&h.to_ascii_lowercase()));
                sutun_hafta = hucreler
                    .iter()
                    .position(|h| haftalik_baslik_mi(&h.to_ascii_lowercase()));
                tablo_used_mi = alt.contains("used") || alt.contains("kullan");
            }
            continue;
        };

        let used_mi = tablo_used_mi || alt.contains("used") || alt.contains("kullan");
        let pencereler = pencere_haritasi.entry(havuz_id).or_default();

        if !hucreler.is_empty() && (sutun_5s.is_some() || sutun_hafta.is_some()) {
            if let Some(idx) = sutun_5s
                && let Some(hucre) = hucreler.get(idx)
                && let Some(&yuzde) = satirdan_yuzdeleri_cikar(hucre).first()
                && havuz_id != "agy_argon"
            {
                let kalan = yuzdeyi_kalana_cevir(yuzde, used_mi);
                pencereler.insert(
                    "5s".to_string(),
                    PencereOlcum {
                        uzunluk_dk: 300,
                        kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                        kalan,
                        sifirlama: None,
                    },
                );
            }
            if let Some(idx) = sutun_hafta
                && let Some(hucre) = hucreler.get(idx)
                && let Some(&yuzde) = satirdan_yuzdeleri_cikar(hucre).first()
            {
                let kalan = yuzdeyi_kalana_cevir(yuzde, used_mi);
                pencereler.insert(
                    "hafta".to_string(),
                    PencereOlcum {
                        uzunluk_dk: 10080,
                        kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                        kalan,
                        sifirlama: None,
                    },
                );
            }
            if !pencereler.is_empty() {
                continue;
            }
        }

        let yuzdeler = satirdan_yuzdeleri_cikar(satir);
        if yuzdeler.is_empty() {
            continue;
        }
        let sifirlama = satirdan_sifirlama_cikar(satir);

        if havuz_id == "agy_argon" {
            let kalan = yuzdeyi_kalana_cevir(yuzdeler[0], used_mi);
            pencereler.insert(
                "hafta".to_string(),
                PencereOlcum {
                    uzunluk_dk: 10080,
                    kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                    kalan,
                    sifirlama,
                },
            );
            continue;
        }

        let has_5s = bes_saatlik_baslik_mi(&alt);
        let has_hafta = haftalik_baslik_mi(&alt);
        if has_5s {
            let kalan = yuzdeyi_kalana_cevir(yuzdeler[0], used_mi);
            pencereler.insert(
                "5s".to_string(),
                PencereOlcum {
                    uzunluk_dk: 300,
                    kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                    kalan,
                    sifirlama,
                },
            );
        }
        if has_hafta {
            let idx = if has_5s && yuzdeler.len() >= 2 { 1 } else { 0 };
            let kalan = yuzdeyi_kalana_cevir(yuzdeler[idx], used_mi);
            pencereler.insert(
                "hafta".to_string(),
                PencereOlcum {
                    uzunluk_dk: 10080,
                    kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                    kalan,
                    sifirlama,
                },
            );
        }
        if pencereler.is_empty() && yuzdeler.len() >= 2 {
            let kalan_5s = yuzdeyi_kalana_cevir(yuzdeler[0], used_mi);
            pencereler.insert(
                "5s".to_string(),
                PencereOlcum {
                    uzunluk_dk: 300,
                    kullanilan: (1.0 - kalan_5s).clamp(0.0, 1.0),
                    kalan: kalan_5s,
                    sifirlama: None,
                },
            );
            let kalan_h = yuzdeyi_kalana_cevir(yuzdeler[1], used_mi);
            pencereler.insert(
                "hafta".to_string(),
                PencereOlcum {
                    uzunluk_dk: 10080,
                    kullanilan: (1.0 - kalan_h).clamp(0.0, 1.0),
                    kalan: kalan_h,
                    sifirlama: None,
                },
            );
        } else if pencereler.is_empty() {
            let kalan = yuzdeyi_kalana_cevir(yuzdeler[0], used_mi);
            pencereler.insert(
                "hafta".to_string(),
                PencereOlcum {
                    uzunluk_dk: 10080,
                    kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                    kalan,
                    sifirlama: None,
                },
            );
        }
    }

    if pencere_haritasi.is_empty() {
        None
    } else {
        Some(havuzlari_insa_et(pencere_haritasi, simdi))
    }
}

fn satirdan_yuzdeleri_cikar(satir: &str) -> Vec<f64> {
    let mut sonuclar = Vec::new();
    let bayt = satir.as_bytes();
    let mut i = 0usize;
    while i < bayt.len() {
        if bayt[i] == b'%' {
            // Önce `%85` biçimi
            let mut j = i + 1;
            while j < bayt.len() && (bayt[j].is_ascii_digit() || bayt[j] == b'.') {
                j += 1;
            }
            if j > i + 1
                && let Ok(v) = satir[i + 1..j].parse::<f64>()
                && (0.0..=100.0).contains(&v)
            {
                sonuclar.push(v);
                i = j;
                continue;
            }
            // Sonra `85%` biçimi
            let mut k = i;
            while k > 0 && (bayt[k - 1].is_ascii_digit() || bayt[k - 1] == b'.') {
                k -= 1;
            }
            if k < i
                && let Ok(v) = satir[k..i].parse::<f64>()
                && (0.0..=100.0).contains(&v)
            {
                sonuclar.push(v);
            }
        }
        i += 1;
    }
    sonuclar
}

fn yuzdeyi_kalana_cevir(yuzde: f64, used_mi: bool) -> f64 {
    let oran = (yuzde / 100.0).clamp(0.0, 1.0);
    if used_mi {
        (1.0 - oran).clamp(0.0, 1.0)
    } else {
        oran
    }
}

fn zaman_degeri_coz(val: &Value) -> Option<UtcZaman> {
    match val {
        Value::String(s) => UtcZaman::parse_tarih_veya_zaman(s).ok(),
        Value::Number(n) => n.as_i64().and_then(UtcZaman::from_unix_timestamp),
        _ => None,
    }
}

fn havuzlari_insa_et(
    pencere_haritasi: BTreeMap<&'static str, BTreeMap<String, PencereOlcum>>,
    simdi: UtcZaman,
) -> BTreeMap<String, HavuzAnlik> {
    let mut sonuc = BTreeMap::new();
    for (havuz_id, pencereler) in pencere_haritasi {
        let etiket = match havuz_id {
            "agy_gemini" => "Antigravity Gemini",
            "agy_claude_gpt" => "Antigravity Claude-GPT",
            "agy_argon" => "Antigravity Argon",
            _ => "Antigravity",
        };
        let mut havuz = HavuzAnlik {
            platform: "agy".to_string(),
            etiket: etiket.to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::AgyUsage,
                guven: GuvenEtiketi::Tahmini,
                okuma: simdi,
                yas_sn: 0,
                hassasiyet: None,
            }),
            pencereler,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: vec!["tek okuma (±%1 oynaklık olabilir)".into()],
        };
        havuz.hesapla_etkin_durum();
        sonuc.insert(havuz_id.to_string(), havuz);
    }
    sonuc
}

fn agy_hata_havuzu(
    etiket: &str,
    durum: Durum,
    hata_turu: &str,
    http_kodu: Option<u16>,
    notlar: Vec<String>,
) -> HavuzAnlik {
    HavuzAnlik {
        platform: "agy".to_string(),
        etiket: etiket.to_string(),
        durum,
        etkin_kalan: None,
        baglayici_pencere: None,
        kullanilabilir: Some(false),
        kaynak: None,
        pencereler: BTreeMap::new(),
        alt_sayaclar: BTreeMap::new(),
        tahmin: BTreeMap::new(),
        baski: None,
        bakiye: None,
        ek: None,
        hata: (durum == Durum::Hata).then(|| HataBilgisi {
            tur: hata_turu.to_string(),
            http_kodu,
        }),
        notlar,
    }
}

fn etiket_from_id(id: &str) -> &'static str {
    match id {
        "agy_gemini" => "Antigravity Gemini",
        "agy_claude_gpt" => "Antigravity Claude-GPT",
        "agy_argon" => "Antigravity Argon",
        _ => "Antigravity",
    }
}

fn kapali_sonuclar(not: &str) -> Vec<BaglayiciSonucu> {
    AGY_HAVUZLARI
        .iter()
        .map(|id| BaglayiciSonucu {
            havuz_id: (*id).to_string(),
            havuz: agy_hata_havuzu(
                etiket_from_id(id),
                Durum::Kapali,
                "kapali",
                None,
                vec![not.to_string()],
            ),
            retry_after_sn: None,
            gecici_hata: false,
        })
        .collect()
}

fn baglanmadi_sonuclari(not: &str) -> Vec<BaglayiciSonucu> {
    AGY_HAVUZLARI
        .iter()
        .map(|id| BaglayiciSonucu {
            havuz_id: (*id).to_string(),
            havuz: agy_hata_havuzu(
                etiket_from_id(id),
                Durum::Baglanmadi,
                "ikili_bulunamadi",
                None,
                vec![not.to_string()],
            ),
            retry_after_sn: None,
            gecici_hata: false,
        })
        .collect()
}

fn hata_sonuclari(
    tur: &str,
    http_kodu: Option<u16>,
    not: &str,
    gecici_hata: bool,
) -> Vec<BaglayiciSonucu> {
    AGY_HAVUZLARI
        .iter()
        .map(|id| hata_sonucu(id, etiket_from_id(id), tur, http_kodu, not, gecici_hata))
        .collect()
}

fn hata_sonucu(
    id: &str,
    etiket: &str,
    tur: &str,
    http_kodu: Option<u16>,
    not: &str,
    gecici_hata: bool,
) -> BaglayiciSonucu {
    BaglayiciSonucu {
        havuz_id: id.to_string(),
        havuz: agy_hata_havuzu(etiket, Durum::Hata, tur, http_kodu, vec![not.to_string()]),
        retry_after_sn: None,
        gecici_hata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
    }

    #[tokio::test]
    async fn varsayilan_kapali_izinle_surec_baslatmaz() {
        let baglayici = AntigravityBaglayicisi::yeni(AgyAyari::default());
        assert_eq!(baglayici.izin(), AgyOkumaIzni::Kapali);

        let sonuclar = baglayici.oku_coklu(z("2026-11-15T18:00:00Z")).await;
        assert_eq!(sonuclar.len(), 3);
        assert!(sonuclar.iter().all(|s| s.havuz.durum == Durum::Kapali));
        assert!(sonuclar.iter().all(|s| !s.gecici_hata));
    }

    #[tokio::test]
    async fn istek_uzerine_ayari_otomatik_okumada_calismaz() {
        let ayar = AgyAyari {
            okuma: AgyOkumaIzni::IstekUzerine,
            ..Default::default()
        };
        let baglayici = AntigravityBaglayicisi::yeni(ayar);
        let otomatik_sonuc = baglayici.oku_coklu(z("2026-11-15T18:00:00Z")).await;
        assert!(
            otomatik_sonuc
                .iter()
                .all(|s| s.havuz.durum == Durum::Kapali)
        );
    }

    #[test]
    fn agy_json_ciktisini_uc_havuza_ayristirir() {
        let json = r#"{
            "command": {
                "data": {
                    "groups": [
                        {
                            "id": "gemini",
                            "buckets": [
                                { "id": "gemini-5h", "window": "5h", "remaining_fraction": 0.94, "reset_time": "2026-11-15T22:00:00Z" },
                                { "id": "gemini-weekly", "window": "weekly", "remaining_fraction": 0.62, "reset_time": "2026-11-20T00:00:00Z" }
                            ]
                        },
                        {
                            "id": "3p",
                            "buckets": [
                                { "id": "3p-5h", "window": "5h", "remaining_fraction": 0.45, "reset_time": "2026-11-15T21:00:00Z" },
                                { "id": "3p-weekly", "window": "weekly", "remaining_fraction": 0.80, "reset_time": "2026-11-20T00:00:00Z" }
                            ]
                        },
                        {
                            "id": "argon",
                            "buckets": [
                                { "id": "gemini-argon-wtu-weekly", "window": "weekly", "remaining_fraction": 0.99, "reset_time": "2026-11-21T00:00:00Z" }
                            ]
                        }
                    ]
                }
            }
        }"#;

        let havuzlar = agy_cikti_ayristir(json, z("2026-11-15T18:00:00Z"));
        assert_eq!(havuzlar.len(), 3);

        let gemini = &havuzlar["agy_gemini"];
        assert_eq!(gemini.durum, Durum::Ok);
        assert_eq!(gemini.baglayici_pencere.as_deref(), Some("hafta"));
        assert!((gemini.etkin_kalan.unwrap() - 0.62).abs() < 1e-9);
        assert_eq!(
            gemini.kaynak.as_ref().map(|k| (k.tur, k.guven)),
            Some((KaynakTuru::AgyUsage, GuvenEtiketi::Tahmini))
        );

        let claude_gpt = &havuzlar["agy_claude_gpt"];
        assert_eq!(claude_gpt.baglayici_pencere.as_deref(), Some("5s"));
        assert!((claude_gpt.etkin_kalan.unwrap() - 0.45).abs() < 1e-9);

        let argon = &havuzlar["agy_argon"];
        assert_eq!(argon.baglayici_pencere.as_deref(), Some("hafta"));
        assert!((argon.etkin_kalan.unwrap() - 0.99).abs() < 1e-9);
    }

    #[test]
    fn agy_metin_tablosunu_yedek_olarak_ayristirir() {
        let tablo = "\
| Model / Havuz | 5s Kalan | Haftalık Kalan |
| Gemini        | %95      | %70            |
| Claude-GPT    | %30      | %85            |
| Argon         | -        | %100           |
";
        let havuzlar = agy_cikti_ayristir(tablo, z("2026-11-15T18:00:00Z"));
        assert_eq!(havuzlar.len(), 3);
        assert!((havuzlar["agy_gemini"].pencereler["5s"].kalan - 0.95).abs() < 1e-9);
        assert!((havuzlar["agy_gemini"].pencereler["hafta"].kalan - 0.70).abs() < 1e-9);
        assert!((havuzlar["agy_claude_gpt"].etkin_kalan.unwrap() - 0.30).abs() < 1e-9);
        assert!((havuzlar["agy_argon"].etkin_kalan.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn markdown_ayirici_satiri_used_basligini_silmez() {
        let tablo = "\
| Model | 5s Used | Haftalik Used |
|---|:---:|---|
| Gemini | %5 | %30 |
| Claude-GPT | %70 | %15 |
";
        let havuzlar = agy_cikti_ayristir(tablo, z("2026-11-15T18:00:00Z"));
        assert!((havuzlar["agy_gemini"].pencereler["5s"].kalan - 0.95).abs() < 1e-9);
        assert!((havuzlar["agy_gemini"].pencereler["hafta"].kalan - 0.70).abs() < 1e-9);
        assert!((havuzlar["agy_claude_gpt"].pencereler["5s"].kalan - 0.30).abs() < 1e-9);
        assert!((havuzlar["agy_claude_gpt"].pencereler["hafta"].kalan - 0.85).abs() < 1e-9);
    }

    #[test]
    fn saatlik_sayac_devralinir() {
        let eski = AntigravityBaglayicisi::yeni(AgyAyari::default());
        let t = z("2026-11-15T18:00:00Z");
        eski.saatlik_cagrilar
            .lock()
            .unwrap()
            .extend([t, t.ekle_sn(60)]);
        let yeni = AntigravityBaglayicisi::yeni(AgyAyari::default());
        yeni.sayaci_devral(&eski);
        assert_eq!(yeni.saatlik_cagrilar.lock().unwrap().len(), 2);
    }

    #[test]
    fn agy_metin_yedeği_five_hour_ve_five_hour_tireli_basliklari_tanir() {
        let girdi = "\
Gemini Models\tFive-Hour Limit Remaining\t80%\t2026-11-16T18:05:17Z
Gemini Models\tWeekly Limit Remaining\t23%\t2026-11-21T15:00:50Z
Claude and GPT models\t5h Limit Remaining\t65%\t2026-11-16T18:05:17Z
";
        let havuzlar = agy_cikti_ayristir(girdi, z("2026-11-16T16:00:00Z"));
        assert_eq!(havuzlar.len(), 2);
        let gemini = &havuzlar["agy_gemini"];
        assert!((gemini.pencereler["5s"].kalan - 0.80).abs() < 1e-9);
        assert!((gemini.pencereler["5s"].kullanilan - 0.20).abs() < 1e-9);
        assert!((gemini.pencereler["hafta"].kalan - 0.23).abs() < 1e-9);
        assert_eq!(
            gemini.pencereler["5s"].sifirlama,
            Some(z("2026-11-16T18:05:17Z"))
        );

        let claude_gpt = &havuzlar["agy_claude_gpt"];
        assert!((claude_gpt.pencereler["5s"].kalan - 0.65).abs() < 1e-9);
    }
}
