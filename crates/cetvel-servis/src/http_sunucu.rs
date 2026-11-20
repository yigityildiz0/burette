use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::header::{AUTHORIZATION, ETAG, HOST, IF_NONE_MATCH, ORIGIN};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use cetvel_model::UtcZaman;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, watch};

use crate::ServisHatasi;
use crate::claude_kopru::KiraIstegi;
use crate::durum::PaylasilanServisDurumu;

/// Köprü yükü büyüyebilir (yeni alt sayaçlar, OAuth `usage` alanları): 4 KB dar kalıyordu.
const AZAMI_GOVDE_BAYT: usize = 16 * 1024;
const AZAMI_ESZAMANLI_ISTEK: usize = 16;
/// `GET /v1/anlik?bekle=` uzun yoklamaları ayrı sınır kullanır: 60 sn bekleyen istemciler genel
/// eşzamanlılık iznini tutup sağlık/yenile/bağla uçlarını 429'a düşürmesin.
const AZAMI_UZUN_YOKLAMA: usize = 8;
const AZAMI_SANIYE_ISTEK: usize = 20;

#[derive(Clone)]
struct HttpGuvenlikDurumu {
    paylasilan: Arc<PaylasilanServisDurumu>,
    eszamanli_kapisi: Arc<Semaphore>,
    uzun_yoklama_kapisi: Arc<Semaphore>,
    hiz_siniri: Arc<Mutex<HizSinirlayici>>,
}

struct HizSinirlayici {
    pencere_baslangic: Instant,
    sayac: usize,
}

impl HizSinirlayici {
    fn yeni() -> Self {
        Self {
            pencere_baslangic: Instant::now(),
            sayac: 0,
        }
    }

    fn izin_var_mi(&mut self) -> bool {
        let simdi = Instant::now();
        if simdi.duration_since(self.pencere_baslangic) >= Duration::from_secs(1) {
            self.pencere_baslangic = simdi;
            self.sayac = 0;
        }
        if self.sayac >= AZAMI_SANIYE_ISTEK {
            return false;
        }
        self.sayac += 1;
        true
    }
}

#[derive(Debug, Deserialize)]
pub struct AnlikSorgu {
    #[serde(default)]
    bekle: Option<u64>,
    #[serde(default)]
    sure: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct GecmisSorgu {
    #[serde(default)]
    gun: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct YenileSorgu {
    #[serde(default)]
    havuz: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestGovdesi {
    platform: String,
}

#[derive(Debug, Serialize)]
struct HataGovdesi {
    hata: String,
    detay: String,
}

/// `127.0.0.1:<port>` dinleyicisi açar; port doluysa ve `otomatik_port` açıksa işletim
/// sisteminden boş port alır (seçilen port `baglanti.json`'a yazılır).
pub async fn loopback_dinleyici_ac(
    port: u16,
    otomatik_port: bool,
) -> Result<TcpListener, ServisHatasi> {
    let addr_v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    match TcpListener::bind(addr_v4).await {
        Ok(l) => Ok(l),
        Err(e) if otomatik_port && port != 0 => {
            tracing::warn!("HTTP 127.0.0.1:{port} kullanılamadı ({e}); boş port seçiliyor");
            TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
                .await
                .map_err(|e2| ServisHatasi::Genel(format!("HTTP boş port açılamadı: {e2}")))
        }
        Err(e) => Err(ServisHatasi::Genel(format!(
            "HTTP 127.0.0.1:{port} bağlanamadı: {e}"
        ))),
    }
}

/// Yerel HTTP sunucusunu hem `127.0.0.1` hem mümkünse `[::1]` üzerinde başlatır.
pub async fn http_sunucusu_baslat(
    durum: Arc<PaylasilanServisDurumu>,
    port: u16,
    otomatik_port: bool,
    kapanis: watch::Receiver<bool>,
) -> Result<(u16, Vec<String>, tokio::task::JoinHandle<()>), ServisHatasi> {
    let listener_v4 = loopback_dinleyici_ac(port, otomatik_port).await?;
    let baglanan_port = listener_v4
        .local_addr()
        .map_err(|e| ServisHatasi::Genel(format!("HTTP yerel port okunamadı: {e}")))?
        .port();
    durum.http_port_ayarla(baglanan_port);

    let mut adresler = vec![format!("http://127.0.0.1:{baglanan_port}")];

    let addr_v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), baglanan_port);
    let listener_v6 = match TcpListener::bind(addr_v6).await {
        Ok(l) => {
            adresler.push(format!("http://[::1]:{baglanan_port}"));
            Some(l)
        }
        Err(e) => {
            tracing::debug!("IPv6 [::1]:{baglanan_port} dinlenemedi ({e}); yalnız IPv4 açık");
            None
        }
    };

    let router = http_router_olustur(durum);

    let gorev = tokio::spawn(async move {
        let mut kap_v4 = kapanis.clone();
        let router_v4 = router.clone();
        let gorev_v4 = tokio::spawn(async move {
            let _ = axum::serve(listener_v4, router_v4)
                .with_graceful_shutdown(async move {
                    while !*kap_v4.borrow() {
                        if kap_v4.changed().await.is_err() {
                            break;
                        }
                    }
                })
                .await;
        });

        let gorev_v6 = listener_v6.map(|l_v6| {
            let mut kap_v6 = kapanis.clone();
            tokio::spawn(async move {
                let _ = axum::serve(l_v6, router)
                    .with_graceful_shutdown(async move {
                        while !*kap_v6.borrow() {
                            if kap_v6.changed().await.is_err() {
                                break;
                            }
                        }
                    })
                    .await;
            })
        });

        let _ = gorev_v4.await;
        if let Some(gv6) = gorev_v6 {
            let _ = gv6.await;
        }
    });

    Ok((baglanan_port, adresler, gorev))
}

pub fn http_router_olustur(paylasilan: Arc<PaylasilanServisDurumu>) -> Router {
    let guvenlik = HttpGuvenlikDurumu {
        paylasilan: Arc::clone(&paylasilan),
        eszamanli_kapisi: Arc::new(Semaphore::new(AZAMI_ESZAMANLI_ISTEK)),
        uzun_yoklama_kapisi: Arc::new(Semaphore::new(AZAMI_UZUN_YOKLAMA)),
        hiz_siniri: Arc::new(Mutex::new(HizSinirlayici::yeni())),
    };

    Router::new()
        .route("/v1/anlik", get(get_anlik))
        .route("/v1/havuz/{ad}", get(get_havuz))
        .route("/v1/ajanlar", get(get_ajanlar))
        .route("/v1/saglik", get(get_saglik))
        .route("/v1/gecmis/{havuz}/{pencere}", get(get_gecmis))
        .route("/v1/yenile", post(post_yenile))
        .route("/v1/bagla/tara", get(get_bagla_tara))
        .route("/v1/bagla/test", post(post_bagla_test))
        .route("/v1/ayar/yenile", post(post_ayar_yenile))
        .route("/v1/claude", post(post_claude))
        .route("/v1/kopru/claude", post(post_claude))
        .route("/v1/claude/kira", post(post_claude_kira))
        .route("/v1/kopru/claude/kira", post(post_claude_kira))
        .layer(DefaultBodyLimit::max(AZAMI_GOVDE_BAYT))
        .layer(middleware::from_fn_with_state(guvenlik, guvenlik_katmani))
        .with_state(paylasilan)
}

async fn guvenlik_katmani(
    State(guvenlik): State<HttpGuvenlikDurumu>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let headers = req.headers();

    // 1. Tarayıcı kaynaklı Origin başlıklarını koşulsuz reddet (CORS yok).
    if headers.contains_key(ORIGIN) {
        return json_hata(
            StatusCode::FORBIDDEN,
            "origin_yasak",
            "Origin başlığına izin verilmez",
        );
    }

    // 2. Host başlığı yalnız loopback olabilir (`127.0.0.1`, `localhost`, `[::1]`).
    if let Some(host_val) = headers.get(HOST) {
        let Ok(host_str) = host_val.to_str() else {
            return json_hata(StatusCode::FORBIDDEN, "host_gecersiz", "Geçersiz Host");
        };
        if !host_loopback_mi(host_str) {
            return json_hata(
                StatusCode::FORBIDDEN,
                "host_yasak",
                "Yalnız yerel loopback Host kabul edilir",
            );
        }
    }

    // 3. Sabit zamanlı Bearer jeton kontrolü.
    let yetkili = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .is_some_and(|gelen| {
            sabit_zamanli_esit(
                gelen.trim().as_bytes(),
                guvenlik.paylasilan.jeton.as_bytes(),
            )
        });
    if !yetkili {
        return json_hata(
            StatusCode::UNAUTHORIZED,
            "yetkisiz",
            "Geçerli Bearer jetonu gerekli",
        );
    }

    // 4. Hız sınırı (<= 20 istek/sn).
    let hiz_uygun = guvenlik
        .hiz_siniri
        .lock()
        .map(|mut h| h.izin_var_mi())
        .unwrap_or(false);
    if !hiz_uygun {
        return json_hata(
            StatusCode::TOO_MANY_REQUESTS,
            "hiz_siniri",
            "İstek hızı sınırı aşıldı (20/sn)",
        );
    }

    // 5. Eşzamanlı istek sınırı (<= 16); uzun yoklama ayrı havuzdan (<= 8).
    let uzun_yoklama = req.uri().path() == "/v1/anlik"
        && req
            .uri()
            .query()
            .is_some_and(|q| q.split('&').any(|p| p.starts_with("bekle=")));
    let kapi = if uzun_yoklama {
        &guvenlik.uzun_yoklama_kapisi
    } else {
        &guvenlik.eszamanli_kapisi
    };
    let Ok(_izin) = Arc::clone(kapi).try_acquire_owned() else {
        return json_hata(
            StatusCode::TOO_MANY_REQUESTS,
            "eszamanli_siniri",
            "Eşzamanlı istek sınırı dolu",
        );
    };

    let yanit = next.run(req).await;
    // Gövde sınırı aşımı düz metin yerine JSON hata döndürür (köprü istemcisi ayrıştırabilsin).
    if yanit.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return json_hata(
            StatusCode::PAYLOAD_TOO_LARGE,
            "govde_cok_buyuk",
            "İstek gövdesi 16 KB sınırını aşıyor",
        );
    }
    yanit
}

fn sabit_zamanli_esit(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

fn host_loopback_mi(host: &str) -> bool {
    let temiz = host.trim().to_ascii_lowercase();
    if temiz.starts_with('[') {
        let Some(kapanis) = temiz.find(']') else {
            return false;
        };
        let ici = &temiz[1..kapanis];
        let kalani = &temiz[kapanis + 1..];
        let port_ok = kalani.is_empty()
            || kalani
                .strip_prefix(':')
                .is_some_and(|p| p.parse::<u16>().is_ok());
        return ici == "::1" && port_ok;
    }
    let ad = temiz.split(':').next().unwrap_or("");
    let port_ok = match temiz.split_once(':') {
        Some((_, p)) => p.parse::<u16>().is_ok(),
        None => true,
    };
    port_ok && (ad == "127.0.0.1" || ad == "localhost")
}

async fn get_anlik(
    State(durum): State<Arc<PaylasilanServisDurumu>>,
    Query(sorgu): Query<AnlikSorgu>,
    headers: HeaderMap,
) -> Response {
    if let Some(beklenen_sira) = sorgu.bekle {
        let mut rx = durum.sira_yayini.subscribe();
        let mevcut = *rx.borrow_and_update();
        if mevcut <= beklenen_sira {
            let sure = Duration::from_secs(sorgu.sure.unwrap_or(25).clamp(1, 60));
            let _ = tokio::time::timeout(sure, async move {
                while *rx.borrow() <= beklenen_sira {
                    if rx.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await;
        }
    }

    let anlik = durum.anlik_kopya().await;
    let etag_deger = format!("\"{}\"", anlik.sira);
    if let Some(inm) = headers.get(IF_NONE_MATCH).and_then(|v| v.to_str().ok()) {
        let temiz_inm = inm.trim().trim_matches('"');
        if temiz_inm == anlik.sira.to_string() {
            let mut resp = StatusCode::NOT_MODIFIED.into_response();
            if let Ok(hv) = HeaderValue::from_str(&etag_deger) {
                resp.headers_mut().insert(ETAG, hv);
            }
            return resp;
        }
    }

    let mut resp = (StatusCode::OK, Json(anlik)).into_response();
    if let Ok(hv) = HeaderValue::from_str(&etag_deger) {
        resp.headers_mut().insert(ETAG, hv);
    }
    resp
}

async fn get_havuz(
    State(durum): State<Arc<PaylasilanServisDurumu>>,
    Path(ad): Path<String>,
) -> Response {
    match durum.havuz_kopya(&ad).await {
        Some(havuz) => (StatusCode::OK, Json(havuz)).into_response(),
        None => json_hata(
            StatusCode::NOT_FOUND,
            "havuz_bulunamadi",
            &format!("Havuz bulunamadı: {ad}"),
        ),
    }
}

/// Claude dışı ajan ağacı (`cetvel-ajan`). `ETag` = `sira`; `If-None-Match` eşleşirse 304.
async fn get_ajanlar(
    State(durum): State<Arc<PaylasilanServisDurumu>>,
    headers: HeaderMap,
) -> Response {
    let ajanlar = durum.ajanlar_kopya();
    let etag_deger = format!("\"ajan-{}\"", ajanlar.sira);
    let eslesti = headers
        .get(IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|inm| inm.trim() == etag_deger);
    let mut resp = if eslesti {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        (StatusCode::OK, Json(&*ajanlar)).into_response()
    };
    if let Ok(hv) = HeaderValue::from_str(&etag_deger) {
        resp.headers_mut().insert(ETAG, hv);
    }
    resp
}

async fn get_saglik(State(durum): State<Arc<PaylasilanServisDurumu>>) -> Response {
    let ozet = durum.saglik_ozeti().await;
    (StatusCode::OK, Json(ozet)).into_response()
}

async fn get_gecmis(
    State(durum): State<Arc<PaylasilanServisDurumu>>,
    Path((havuz, pencere)): Path<(String, String)>,
    Query(sorgu): Query<GecmisSorgu>,
) -> Response {
    let gun = sorgu.gun.unwrap_or(7);
    let yanit = durum.gecmis_sorgula(&havuz, &pencere, gun, UtcZaman::simdi());
    (StatusCode::OK, Json(yanit)).into_response()
}

async fn post_yenile(
    State(durum): State<Arc<PaylasilanServisDurumu>>,
    Query(sorgu): Query<YenileSorgu>,
    govde: Option<Json<YenileSorgu>>,
) -> Response {
    let havuz = sorgu.havuz.or_else(|| govde.and_then(|Json(g)| g.havuz));
    let yanit = durum.yenile_iste(havuz.as_deref(), UtcZaman::simdi()).await;
    let kod = if yanit.istendi {
        StatusCode::ACCEPTED
    } else {
        StatusCode::TOO_MANY_REQUESTS
    };
    (kod, Json(yanit)).into_response()
}

async fn get_bagla_tara(State(durum): State<Arc<PaylasilanServisDurumu>>) -> Response {
    (StatusCode::OK, Json(durum.bagla_tara().await)).into_response()
}

async fn post_bagla_test(
    State(durum): State<Arc<PaylasilanServisDurumu>>,
    Json(govde): Json<TestGovdesi>,
) -> Response {
    match durum.bagla_test(&govde.platform).await {
        Ok(sonuc) => (StatusCode::OK, Json(sonuc)).into_response(),
        Err(e) => json_hata(StatusCode::BAD_REQUEST, "test_hatasi", &e),
    }
}

async fn post_ayar_yenile(State(durum): State<Arc<PaylasilanServisDurumu>>) -> Response {
    durum.ayar_yenile_iste.notify_one();
    (
        StatusCode::ACCEPTED,
        Json(json!({ "istendi": true, "uyari": durum.ayar_uyarisi() })),
    )
        .into_response()
}

async fn post_claude(
    State(durum): State<Arc<PaylasilanServisDurumu>>,
    Json(yuk): Json<Value>,
) -> Response {
    match durum.claude_guncelle(yuk, UtcZaman::simdi()).await {
        Ok(yanit) => (StatusCode::OK, Json(yanit)).into_response(),
        Err(hata) => json_hata(
            StatusCode::BAD_REQUEST,
            "gecersiz_claude_verisi",
            &hata.to_string(),
        ),
    }
}

async fn post_claude_kira(
    State(durum): State<Arc<PaylasilanServisDurumu>>,
    Json(istek): Json<KiraIstegi>,
) -> Response {
    let sonuc = durum.claude_kira_talep(&istek, UtcZaman::simdi());
    let kod = if sonuc.verildi {
        StatusCode::OK
    } else if sonuc.sahip.is_empty() {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::CONFLICT
    };
    (kod, Json(sonuc)).into_response()
}

fn json_hata(kod: StatusCode, hata: &str, detay: &str) -> Response {
    (
        kod,
        Json(HataGovdesi {
            hata: hata.to_string(),
            detay: detay.to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cetvel_depo::{Ayarlar, GecmisDeposu};
    use cetvel_model::{Durum, HavuzAnlik, PencereOlcum};
    use serde_json::json;

    use super::*;

    async fn test_sunucusu_kur() -> (
        u16,
        Arc<PaylasilanServisDurumu>,
        watch::Sender<bool>,
        tokio::task::JoinHandle<()>,
    ) {
        let mut depo = GecmisDeposu::bellekte().unwrap();
        let simdi = UtcZaman::simdi();
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 0.25,
                kalan: 0.75,
                sifirlama: Some(simdi.ekle_sn(7200)),
                etiket: None,
            },
        );
        let mut codex = HavuzAnlik {
            platform: "codex".to_string(),
            etiket: "Codex Plus".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
            kesif: None,
            etkin_kalan_zincir: None,
        };
        codex.hesapla_etkin_durum();

        let anlik = cetvel_model::AnlikV2::yeni(
            simdi,
            1,
            BTreeMap::from([("codex_plus".to_string(), codex)]),
        );
        depo.anlik_kaydet(&anlik).unwrap();

        let durum = Arc::new(PaylasilanServisDurumu::yeni(
            Ayarlar::default(),
            None,
            "test-gizli-jeton-1234567890abcdef",
            r"\\.\pipe\cetvel-test",
            Some(depo),
        ));
        durum.anlik_yayinla(anlik, simdi).await;

        let (kap_tx, kap_rx) = watch::channel(false);
        let (port, _adresler, gorev) = http_sunucusu_baslat(Arc::clone(&durum), 0, false, kap_rx)
            .await
            .expect("http port 0 baglanmali");
        (port, durum, kap_tx, gorev)
    }

    #[tokio::test]
    async fn yetki_origin_ve_tum_uc_noktalar_gercek_port_sifirda_dogrulanir() {
        let (port, _durum, kap_tx, gorev) = test_sunucusu_kur().await;
        let taban = format!("http://127.0.0.1:{port}");
        let istemci = reqwest::Client::new();
        let jeton = "test-gizli-jeton-1234567890abcdef";

        // 1. Jetonsuz -> 401
        let y_yetkisiz = istemci
            .get(format!("{taban}/v1/anlik"))
            .send()
            .await
            .unwrap();
        assert_eq!(y_yetkisiz.status(), StatusCode::UNAUTHORIZED);

        // 2. Origin başlıklı -> 403
        let y_origin = istemci
            .get(format!("{taban}/v1/anlik"))
            .bearer_auth(jeton)
            .header("Origin", "https://saldirgan.example.invalid")
            .send()
            .await
            .unwrap();
        assert_eq!(y_origin.status(), StatusCode::FORBIDDEN);
        assert!(
            y_origin
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );

        // 3. GET /v1/saglik -> 200
        let y_saglik = istemci
            .get(format!("{taban}/v1/saglik"))
            .bearer_auth(jeton)
            .send()
            .await
            .unwrap();
        assert_eq!(y_saglik.status(), StatusCode::OK);
        let saglik_metin = y_saglik.text().await.unwrap();
        let saglik_json: Value = serde_json::from_str(&saglik_metin).unwrap();
        assert_eq!(saglik_json["durum"], "ok");
        assert_eq!(saglik_json["port"], port);

        // 4. GET /v1/anlik + ETag -> 304
        let y_anlik = istemci
            .get(format!("{taban}/v1/anlik"))
            .bearer_auth(jeton)
            .send()
            .await
            .unwrap();
        assert_eq!(y_anlik.status(), StatusCode::OK);
        let etag = y_anlik
            .headers()
            .get(ETAG)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let y_304 = istemci
            .get(format!("{taban}/v1/anlik"))
            .bearer_auth(jeton)
            .header("If-None-Match", etag)
            .send()
            .await
            .unwrap();
        assert_eq!(y_304.status(), StatusCode::NOT_MODIFIED);

        // 5. GET /v1/havuz/codex_plus
        let y_havuz = istemci
            .get(format!("{taban}/v1/havuz/codex_plus"))
            .bearer_auth(jeton)
            .send()
            .await
            .unwrap();
        assert_eq!(y_havuz.status(), StatusCode::OK);
        let h_json: Value = serde_json::from_str(&y_havuz.text().await.unwrap()).unwrap();
        assert_eq!(h_json["etkin_kalan"], 0.75);

        // 6. POST /v1/claude/kira iki sohbet yarışması
        let k1 = istemci
            .post(format!("{taban}/v1/claude/kira"))
            .bearer_auth(jeton)
            .header("content-type", "application/json")
            .body(json!({ "istemci": "sohbet-a" }).to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(k1.status(), StatusCode::OK);
        let k2 = istemci
            .post(format!("{taban}/v1/claude/kira"))
            .bearer_auth(jeton)
            .header("content-type", "application/json")
            .body(json!({ "istemci": "sohbet-b" }).to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(k2.status(), StatusCode::CONFLICT);

        // 7. POST /v1/claude
        let simdi = UtcZaman::simdi();
        let bitis_5s = simdi.ekle_sn(3600).to_iso_z();
        let y_claude = istemci
            .post(format!("{taban}/v1/claude"))
            .bearer_auth(jeton)
            .header("content-type", "application/json")
            .body(
                json!({
                    "plan": { "status": "max_20x" },
                    "windows": [
                        { "label": "Current session", "percentUsed": 22.0, "resetsAt": bitis_5s }
                    ]
                })
                .to_string(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(y_claude.status(), StatusCode::OK);
        let claude_json: Value = serde_json::from_str(&y_claude.text().await.unwrap()).unwrap();
        assert_eq!(claude_json["uygulandi"], true);

        let y_cm = istemci
            .get(format!("{taban}/v1/havuz/claude_max"))
            .bearer_auth(jeton)
            .send()
            .await
            .unwrap();
        assert_eq!(y_cm.status(), StatusCode::OK);

        // 8. POST /v1/yenile: ilk kabul (202), aynı havuz hemen yeniden 429 + bekle_sn
        let r1 = istemci
            .post(format!("{taban}/v1/yenile?havuz=codex_plus"))
            .bearer_auth(jeton)
            .send()
            .await
            .unwrap();
        assert_eq!(r1.status(), StatusCode::ACCEPTED);
        let r2 = istemci
            .post(format!("{taban}/v1/yenile"))
            .bearer_auth(jeton)
            .header("content-type", "application/json")
            .body(json!({ "havuz": "codex_plus" }).to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(r2.status(), StatusCode::TOO_MANY_REQUESTS);
        let r2_json: Value = serde_json::from_str(&r2.text().await.unwrap()).unwrap();
        assert!(r2_json["bekle_sn"].as_u64().is_some());

        // 9. Bağla taraması yalnız varlık alanları döndürür
        let t = istemci
            .get(format!("{taban}/v1/bagla/tara"))
            .bearer_auth(jeton)
            .send()
            .await
            .unwrap();
        assert_eq!(t.status(), StatusCode::OK);
        let t_json: Value = serde_json::from_str(&t.text().await.unwrap()).unwrap();
        assert!(t_json["codex_durum"].is_string());
        assert!(t_json["agy_okuma"].is_string());

        let _ = kap_tx.send(true);
        let _ = gorev.await;
    }

    #[tokio::test]
    async fn ajanlar_ucu_ayni_yetki_kurallariyla_ve_etag_ile_doner() {
        let (port, durum, kap_tx, gorev) = test_sunucusu_kur().await;
        let url = format!("http://127.0.0.1:{port}/v1/ajanlar");
        let istemci = reqwest::Client::new();
        let jeton = "test-gizli-jeton-1234567890abcdef";

        let y = istemci.get(&url).send().await.unwrap();
        assert_eq!(y.status(), StatusCode::UNAUTHORIZED);
        let y = istemci
            .get(&url)
            .bearer_auth(jeton)
            .header("Origin", "https://saldirgan.example.invalid")
            .send()
            .await
            .unwrap();
        assert_eq!(y.status(), StatusCode::FORBIDDEN);

        let y = istemci.get(&url).bearer_auth(jeton).send().await.unwrap();
        assert_eq!(y.status(), StatusCode::OK);
        let govde: Value = serde_json::from_str(&y.text().await.unwrap()).unwrap();
        assert_eq!(govde["surum"], 1);
        assert_eq!(govde["sira"], 0);
        assert_eq!(govde["ajanlar"], json!([]));

        let simdi = UtcZaman::simdi();
        let mut anlik = cetvel_ajan::AjanAnlik::bos(simdi);
        anlik.ajanlar.push(cetvel_ajan::Ajan {
            kimlik: cetvel_ajan::kimlik_karma(cetvel_ajan::Platform::Codex, "t"),
            platform: cetvel_ajan::Platform::Codex,
            ebeveyn: None,
            derinlik: 0,
            ad: "Codex exec".to_string(),
            model: Some("gpt-test".to_string()),
            efor: None,
            durum: cetvel_ajan::AjanDurumu::Hata,
            etkinlik: Some(cetvel_ajan::Etkinlik::Derleme),
            hata_turu: Some(cetvel_ajan::HataTuru::HizSiniri),
            baslama: Some(simdi),
            son_etkinlik: simdi,
            bitis: Some(simdi),
            sayilar: cetvel_ajan::AjanSayilari::default(),
        });
        let (degisti, _) = durum.ajanlar_guncelle(anlik);
        assert!(degisti);

        let y = istemci.get(&url).bearer_auth(jeton).send().await.unwrap();
        assert_eq!(y.headers()[ETAG], "\"ajan-1\"");
        let govde: Value = serde_json::from_str(&y.text().await.unwrap()).unwrap();
        assert_eq!(govde["sira"], 1);
        let a = &govde["ajanlar"][0];
        assert_eq!(a["platform"], "codex");
        assert_eq!(a["durum"], "hata");
        assert_eq!(a["etkinlik"], "derleme");
        assert_eq!(a["hata_turu"], "429");

        let y = istemci
            .get(&url)
            .bearer_auth(jeton)
            .header("If-None-Match", "\"ajan-1\"")
            .send()
            .await
            .unwrap();
        assert_eq!(y.status(), StatusCode::NOT_MODIFIED);

        let _ = kap_tx.send(true);
        let _ = gorev.await;
    }

    #[tokio::test]
    async fn dolu_portta_otomatik_bos_port_secilir() {
        let mesgul = TcpListener::bind("127.0.0.1:0").await.expect("mesgul port");
        let dolu = mesgul.local_addr().unwrap().port();
        assert!(loopback_dinleyici_ac(dolu, false).await.is_err());
        let yeni = loopback_dinleyici_ac(dolu, true).await.expect("bos port");
        let yeni_port = yeni.local_addr().unwrap().port();
        assert_ne!(yeni_port, dolu);
        assert_ne!(yeni_port, 0);
    }
}
