//! Cetvel MCP stdio JSON-RPC 2.0 sunucusu (`plan.md §5.5`).

use std::future::Future;
use std::pin::Pin;

use cetvel_depo::CetvelYollar;
use cetvel_model::{AnlikV2, Durum, HavuzAnlik, UtcZaman};
use cetvel_servis::servis_pipe_istegi;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

const MCP_PROTOKOL_SURUMU: &str = "2024-11-05";

#[derive(Debug, Error)]
pub enum McpHatasi {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("Anlık kota verisi okunamadı: {0}")]
    VeriYok(String),
}

/// Anlık v2 belgesi sağlayıcı arayüzü (üretimde named pipe + anlik.json, testlerde bellek içi).
pub trait AnlikSaglayici: Send + Sync {
    fn anlik_al(&self) -> Pin<Box<dyn Future<Output = Result<AnlikV2, McpHatasi>> + Send + '_>>;
}

/// Önce yerel (oturuma özgü) named pipe sunucusundan okur; erişemezse `%LOCALAPPDATA%\Cetvel\anlik.json`'a düşer.
/// Pipe adı her istekte yeniden keşfedilir (`baglanti.json` → SID'li ad → eski sabit ad): MCP
/// sunucusu servisten önce başlamış ya da servis yeniden başlamış olsa da doğru pipe'a ulaşır.
pub struct PipeVeDosyaSaglayici {
    pub yollar: CetvelYollar,
}

impl PipeVeDosyaSaglayici {
    pub fn yeni(yollar: CetvelYollar) -> Self {
        Self { yollar }
    }
}

impl AnlikSaglayici for PipeVeDosyaSaglayici {
    fn anlik_al(&self) -> Pin<Box<dyn Future<Output = Result<AnlikV2, McpHatasi>> + Send + '_>> {
        Box::pin(async move {
            if let Ok(deger) = servis_pipe_istegi(&self.yollar, "anlik.al", None).await
                && let Ok(anlik) = serde_json::from_value::<AnlikV2>(deger)
            {
                return Ok(anlik);
            }

            let yol = self.yollar.anlik_dosyasi();
            let icerik = std::fs::read_to_string(&yol).map_err(|e| {
                McpHatasi::VeriYok(format!("pipe kapalı ve {} okunamadı ({e})", yol.display()))
            })?;
            let mut anlik = serde_json::from_str::<AnlikV2>(&icerik)
                .map_err(|e| McpHatasi::VeriYok(format!("anlik.json JSON hatası: {e}")))?;
            // Eşik: en az 5 dk, kalp atışı uzun ayarlandıysa 3 kalp atışı (ayar yan etkisiz okunur).
            let kalp_sn =
                cetvel_depo::Ayarlar::yukle_veya_varsayilan(&self.yollar.ayarlar_dosyasi())
                    .map(|a| a.servis.kalp_atisi_sn)
                    .unwrap_or(60);
            let esik = DOSYA_BAYAT_ESIGI_SN.max(kalp_sn as i64 * 3);
            dosya_anligini_bayatlat(&mut anlik, UtcZaman::simdi(), esik);
            Ok(anlik)
        })
    }
}

/// Servis yanıt vermezken okunan `anlik.json` bu yaştan eskiyse (servis kalp atışı en fazla
/// birkaç dakikada bir yazar) tüm havuzlar bayat sayılır.
pub const DOSYA_BAYAT_ESIGI_SN: i64 = 300;

/// Pipe yerine diskteki `anlik.json`'a düşüldüğünde dosya eskiyse her havuzu `bayat` ve
/// kullanılamaz işaretler: çökmüş servisin donmuş "ok / %60" kaydı `uygun_havuzlar`'da
/// uygun görünmesin. Bayatlatıldıysa `true` döner.
pub fn dosya_anligini_bayatlat(anlik: &mut AnlikV2, simdi: UtcZaman, esik_sn: i64) -> bool {
    let yas = simdi.fark_sn(anlik.zaman);
    if yas <= esik_sn {
        return false;
    }
    let not = format!("servis yanıt vermiyor; anlik.json {yas} sn önce yazıldı (bayat dosya)");
    for havuz in anlik.havuzlar.values_mut() {
        if havuz.durum == Durum::Ok {
            havuz.durum = Durum::Bayat;
        }
        havuz.kullanilabilir = Some(false);
        if let Some(k) = havuz.kaynak.as_mut() {
            k.yas_sn = simdi.fark_sn(k.okuma).max(0) as u64;
        }
        if !havuz.notlar.contains(&not) {
            havuz.notlar.push(not.clone());
        }
    }
    true
}

/// Canlı `cetvel mcp` stdio sunucusunu başlatır (stdout yalnızca JSON-RPC 2.0 içindir).
pub async fn mcp_stdio_sunucusu(yollar: CetvelYollar) -> Result<(), McpHatasi> {
    let saglayici = PipeVeDosyaSaglayici::yeni(yollar);
    let stdin = BufReader::new(tokio::io::stdin());
    let stdout = tokio::io::stdout();
    mcp_stdio_calistir(stdin, stdout, &saglayici).await
}

#[derive(Debug, Deserialize)]
struct RpcIstek {
    #[serde(default)]
    jsonrpc: Option<String>,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

/// Herhangi bir satır tabanlı akış (`tokio::io::stdin`/`stdout` veya test `duplex`) üstünde MCP döngüsü çalıştırır.
pub async fn mcp_stdio_calistir<R, W, S>(
    mut okuyucu: R,
    mut yazici: W,
    saglayici: &S,
) -> Result<(), McpHatasi>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
    S: AnlikSaglayici + ?Sized,
{
    let mut satir = String::new();
    loop {
        satir.clear();
        let n = okuyucu.read_line(&mut satir).await?;
        if n == 0 {
            break;
        }
        let temiz = satir.trim();
        if temiz.is_empty() {
            continue;
        }

        let yanit_opt = rpc_mesajini_isle(temiz, saglayici).await;
        if let Some(yanit) = yanit_opt {
            let mut cikti = serde_json::to_string(&yanit).unwrap_or_else(|_| {
                r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"serileştirme hatası"}}"#
                    .to_string()
            });
            cikti.push('\n');
            yazici.write_all(cikti.as_bytes()).await?;
            yazici.flush().await?;
        }
    }
    Ok(())
}

async fn rpc_mesajini_isle<S: AnlikSaglayici + ?Sized>(ham: &str, saglayici: &S) -> Option<Value> {
    let istek: RpcIstek = match serde_json::from_str(ham) {
        Ok(i) => i,
        Err(e) => {
            return Some(rpc_hata(
                Value::Null,
                -32700,
                &format!("JSON ayrıştırma hatası: {e}"),
            ));
        }
    };

    // Bildirim (id yok) -> JSON-RPC 2.0 gereği yanıt dönülmez.
    let id = match istek.id {
        Some(id) => id,
        None => return None,
    };

    if istek.jsonrpc.as_deref() != Some("2.0") {
        return Some(rpc_hata(id, -32600, "jsonrpc alanı \"2.0\" olmalıdır"));
    }

    match istek.method.as_str() {
        "initialize" => {
            let istemci_protokol = istek
                .params
                .as_ref()
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Value::as_str)
                .unwrap_or(MCP_PROTOKOL_SURUMU);
            Some(rpc_basari(
                id,
                json!({
                    "protocolVersion": istemci_protokol,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "subscribe": false, "listChanged": false }
                    },
                    "serverInfo": {
                        "name": "cetvel",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ))
        }
        "ping" => Some(rpc_basari(id, json!({}))),
        "tools/list" => Some(rpc_basari(id, json!({ "tools": arac_tanimlari() }))),
        "tools/call" => {
            let yanit = arac_cagir(istek.params.unwrap_or(Value::Null), saglayici).await;
            Some(match yanit {
                Ok(v) => rpc_basari(id, v),
                Err((kod, mesaj)) => rpc_hata(id, kod, &mesaj),
            })
        }
        "resources/list" => Some(rpc_basari(
            id,
            json!({
                "resources": [{
                    "uri": "kota://anlik",
                    "name": "Cetvel Anlık Kota v2",
                    "description": "Tüm AI abonelik ve API kotalarının anlık durumu",
                    "mimeType": "application/json"
                }]
            }),
        )),
        "resources/read" => {
            let uri = istek
                .params
                .as_ref()
                .and_then(|p| p.get("uri"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if uri != "kota://anlik" {
                return Some(rpc_hata(id, -32602, &format!("Bilinmeyen kaynak: {uri}")));
            }
            match saglayici.anlik_al().await {
                Ok(anlik) => {
                    let metin = serde_json::to_string_pretty(&anlik).unwrap_or_default();
                    Some(rpc_basari(
                        id,
                        json!({
                            "contents": [{
                                "uri": "kota://anlik",
                                "mimeType": "application/json",
                                "text": metin
                            }]
                        }),
                    ))
                }
                Err(e) => Some(rpc_hata(id, -32000, &e.to_string())),
            }
        }
        diger => Some(rpc_hata(id, -32601, &format!("Bilinmeyen yöntem: {diger}"))),
    }
}

fn arac_tanimlari() -> Value {
    json!([
        {
            "name": "kota_anlik",
            "description": "Tüm tanımlı yapay zeka havuzlarının güncel anlık kota ve tahmin özetini döndürür.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        },
        {
            "name": "havuz_detay",
            "description": "Belirtilen tek bir kota havuzunun pencere ve tahmin ayrıntılarını döndürür.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "havuz": {
                        "type": "string",
                        "description": "Havuz kimliği (ör. codex_plus, claude_max, deepseek, agy_gemini)"
                    }
                },
                "required": ["havuz"],
                "additionalProperties": false
            }
        },
        {
            "name": "uygun_havuzlar",
            "description": "Şu anda kullanılabilir ve etkin_kalan > 0 (veya kullanılabilir bakiyesi olan) havuzları en yüksekten aza listeler.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }
    ])
}

async fn arac_cagir<S: AnlikSaglayici + ?Sized>(
    params: Value,
    saglayici: &S,
) -> Result<Value, (i64, String)> {
    let ad = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "tools/call için name alanı zorunlu".to_string()))?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match ad {
        "kota_anlik" => match saglayici.anlik_al().await {
            Ok(anlik) => arac_metin_sonucu(&anlik, false),
            Err(e) => arac_metin_sonucu(&json!({ "hata": e.to_string() }), true),
        },
        "havuz_detay" => {
            let havuz_ad = args
                .get("havuz")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or((
                    -32602,
                    "havuz_detay için 'havuz' parametresi gerekli".to_string(),
                ))?;
            let anlik = match saglayici.anlik_al().await {
                Ok(a) => a,
                Err(e) => return arac_metin_sonucu(&json!({ "hata": e.to_string() }), true),
            };
            match anlik.havuzlar.get(havuz_ad) {
                Some(h) => arac_metin_sonucu(&json!({ "havuz": havuz_ad, "detay": h }), false),
                None => arac_metin_sonucu(
                    &json!({ "hata": format!("Havuz bulunamadı: {havuz_ad}") }),
                    true,
                ),
            }
        }
        "uygun_havuzlar" => {
            let anlik = match saglayici.anlik_al().await {
                Ok(a) => a,
                Err(e) => return arac_metin_sonucu(&json!({ "hata": e.to_string() }), true),
            };
            let uygunlar = uygun_havuzlari_filtrele(&anlik);
            arac_metin_sonucu(
                &json!({
                    "zaman": anlik.zaman,
                    "sira": anlik.sira,
                    "uygun_havuzlar": uygunlar
                }),
                false,
            )
        }
        diger => Err((-32602, format!("Bilinmeyen araç: {diger}"))),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UygunHavuzOzeti {
    pub havuz: String,
    pub platform: String,
    pub etiket: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etkin_kalan: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baglayici_pencere: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baski: Option<cetvel_model::Baski>,
}

/// `durum == Ok` ve (`etkin_kalan > 0` veya kullanılabilir pozitif bakiye) olan havuzları azalan sırada döndürür.
pub fn uygun_havuzlari_filtrele(anlik: &AnlikV2) -> Vec<UygunHavuzOzeti> {
    let mut liste: Vec<(f64, UygunHavuzOzeti)> = anlik
        .havuzlar
        .iter()
        .filter(|(_, h)| havuz_uygun_mu(h))
        .map(|(ad, h)| {
            let siralama = h.etkin_kalan.unwrap_or(1.0);
            (
                siralama,
                UygunHavuzOzeti {
                    havuz: ad.clone(),
                    platform: h.platform.clone(),
                    etiket: h.etiket.clone(),
                    etkin_kalan: h.etkin_kalan,
                    baglayici_pencere: h.baglayici_pencere.clone(),
                    baski: h.baski,
                },
            )
        })
        .collect();

    liste.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.havuz.cmp(&b.1.havuz))
    });
    liste.into_iter().map(|(_, item)| item).collect()
}

fn havuz_uygun_mu(h: &HavuzAnlik) -> bool {
    if h.durum != Durum::Ok || h.kullanilabilir == Some(false) {
        return false;
    }
    if let Some(kalan) = h.etkin_kalan {
        return kalan > 0.0;
    }
    h.bakiye
        .as_ref()
        .is_some_and(|b| b.toplam > 0.0 && b.kullanilabilir.unwrap_or(true))
}

fn arac_metin_sonucu<T: Serialize>(veri: &T, is_error: bool) -> Result<Value, (i64, String)> {
    let metin = serde_json::to_string_pretty(veri).unwrap_or_else(|_| "{}".to_string());
    Ok(json!({
        "content": [{
            "type": "text",
            "text": metin
        }],
        "isError": is_error
    }))
}

fn rpc_basari(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn rpc_hata(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cetvel_model::{BakiyeBilgisi, PencereOlcum};

    use super::*;

    struct SabitSaglayici {
        anlik: AnlikV2,
    }

    impl AnlikSaglayici for SabitSaglayici {
        fn anlik_al(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<AnlikV2, McpHatasi>> + Send + '_>> {
            Box::pin(async { Ok(self.anlik.clone()) })
        }
    }

    fn havuz_olustur(platform: &str, durum: Durum, kalan: f64) -> HavuzAnlik {
        let mut h = HavuzAnlik {
            platform: platform.to_string(),
            etiket: platform.to_string(),
            durum,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler: BTreeMap::from([(
                "5s".to_string(),
                PencereOlcum {
                    uzunluk_dk: 300,
                    kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                    kalan,
                    sifirlama: None,
                },
            )]),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        h.hesapla_etkin_durum();
        h
    }

    #[test]
    fn eski_dosya_yedegi_bayat_sayilir_ve_uygun_havuz_dondurmez() {
        let simdi = UtcZaman::parse_tarih_veya_zaman("2026-11-15T18:00:00Z").unwrap();
        let havuzlar = BTreeMap::from([(
            "claude_max".to_string(),
            havuz_olustur("claude", Durum::Ok, 0.6),
        )]);
        let mut eski = AnlikV2::yeni(simdi.ekle_sn(-3 * 3600), 40, havuzlar.clone());
        assert_eq!(uygun_havuzlari_filtrele(&eski).len(), 1);
        assert!(dosya_anligini_bayatlat(
            &mut eski,
            simdi,
            DOSYA_BAYAT_ESIGI_SN
        ));
        assert_eq!(eski.havuzlar["claude_max"].durum, Durum::Bayat);
        assert!(uygun_havuzlari_filtrele(&eski).is_empty());

        // Taze dosya (servis yeni yazdı) olduğu gibi kalır.
        let mut taze = AnlikV2::yeni(simdi.ekle_sn(-60), 41, havuzlar);
        assert!(!dosya_anligini_bayatlat(
            &mut taze,
            simdi,
            DOSYA_BAYAT_ESIGI_SN
        ));
        assert_eq!(uygun_havuzlari_filtrele(&taze).len(), 1);
    }

    #[tokio::test]
    async fn mcp_stdio_tum_araclari_ve_uygun_havuzlari_dogru_dondurur() {
        let simdi = UtcZaman::parse_tarih_veya_zaman("2026-11-15T18:00:00Z").unwrap();
        let mut havuzlar = BTreeMap::new();
        havuzlar.insert(
            "codex_plus".to_string(),
            havuz_olustur("codex", Durum::Ok, 0.0),
        );
        havuzlar.insert(
            "claude_max".to_string(),
            havuz_olustur("claude", Durum::Ok, 0.65),
        );
        havuzlar.insert(
            "agy_gemini".to_string(),
            havuz_olustur("agy", Durum::Bayat, 0.90),
        );
        let mut ds = HavuzAnlik {
            platform: "deepseek".to_string(),
            etiket: "DeepSeek".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler: BTreeMap::new(),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: Some(BakiyeBilgisi {
                para_birimi: "USD".to_string(),
                toplam: 10.0,
                hibe: None,
                yuklenen: Some(10.0),
                kullanilabilir: Some(true),
            }),
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        ds.hesapla_etkin_durum();
        havuzlar.insert("deepseek".to_string(), ds);

        let saglayici = SabitSaglayici {
            anlik: AnlikV2::yeni(simdi, 7, havuzlar),
        };

        let istekler = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"kota_anlik","arguments":{}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"havuz_detay","arguments":{"havuz":"claude_max"}}}),
            json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"uygun_havuzlar","arguments":{}}}),
            json!({"jsonrpc":"2.0","id":6,"method":"resources/read","params":{"uri":"kota://anlik"}}),
        ]
        .iter()
        .map(|v| format!("{v}\n"))
        .collect::<String>();

        let mut cikti = Vec::new();
        mcp_stdio_calistir(BufReader::new(istekler.as_bytes()), &mut cikti, &saglayici)
            .await
            .unwrap();

        let satirlar: Vec<Value> = String::from_utf8(cikti)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        // notifications/initialized için yanıt üretilmez -> 6 yanıt beklenir
        assert_eq!(satirlar.len(), 6);
        assert_eq!(satirlar[0]["result"]["serverInfo"]["name"], "cetvel");

        let arac_isimleri: Vec<&str> = satirlar[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            arac_isimleri,
            vec!["kota_anlik", "havuz_detay", "uygun_havuzlar"]
        );

        let detay_metin = satirlar[3]["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(detay_metin.contains("claude_max"));

        let uygun_metin = satirlar[4]["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let uygun_json: Value = serde_json::from_str(uygun_metin).unwrap();
        let uygun_adlar: Vec<&str> = uygun_json["uygun_havuzlar"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| u["havuz"].as_str().unwrap())
            .collect();
        assert!(!uygun_adlar.contains(&"codex_plus")); // %0 kalan elenir
        assert!(!uygun_adlar.contains(&"agy_gemini")); // bayat elenir
        assert!(uygun_adlar.contains(&"claude_max"));
        assert!(uygun_adlar.contains(&"deepseek"));

        let kaynak_metin = satirlar[5]["result"]["contents"][0]["text"]
            .as_str()
            .unwrap();
        assert!(kaynak_metin.contains("\"sira\": 7"));
    }
}
