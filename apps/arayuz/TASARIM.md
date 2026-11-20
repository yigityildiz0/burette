# Cetvel Masaüstü Arayüzü — Tasarım Notu

Bu belge `apps/arayuz/` altındaki arayüzün görsel dilini, görünümlerini, duyarlı düzen kurallarını, çok dilli altyapıyı ve Tauri 2 masaüstü köprüsünü özetler.

## 0. İlke: az metin, tek vurgu, ince çubuk

Önceki sürüm çok rozetli ve açıklama cümlesi yoğun olduğu için göz yoruyordu. Yeni dil, menü çubuğu kota izleyicilerinden ilham alır (kopyalamaz, marka logosu kullanmaz):

- **Tek koyu, yumuşak camsı kart:** açılır pencere tek yüzeydir; içindeki kartlar yarı saydam (`--cam`) ve ince kenarlıklıdır (`--cam-kenar`), köşeler 14–16 px.
- **Her yüzde kalan olarak yazılır ve "kaldı" der** (`genel.kaldi`: `%15 kaldı`). Ayarlar → Genel → *Ana yüzde ölçütü* ile `%85 kullanıldı` gösterimine geçilir (`durum.degerYonu`).
- **Çubuklar ince ve yuvarlak uçlu:** kompakt kartta ve sekme çizgisinde 4 px, ayrıntıda 6 px (`.ince-cubuk`, `.ince-cubuk.kalin`).
- **Tek vurgu rengi:** dolgu her temada `--vurgu`; yalnız eşiğin altındaki pencere `--durum-kacin-bar`, bayat veri `--durum-bayat`, başka pencere yüzünden kilitli olan pencere soluk gri (`.kilitli`). Yüzdelerin arkasında renkli rozet yoktur; kritik değerde yalnız metin rengi değişir.
- **Açıklamalar ipucunda:** "belirleyici", "deneysel", kaynak/güven, tahmin gerekçesi, kilit açıklaması gibi ayrıntılar `data-ipucu` ile hover/odakta görünür. Geçmiş ve Bağla başlıklarındaki uzun açıklama cümlesi başlığın yanındaki bilgi simgesine taşındı (`baslikBilgiYaz`).

## 1. Renk Temaları ve Tokenlar

Tüm renkler `stil.css` içinde CSS değişkenidir. Kip `html[data-mod="acik" | "koyu"]`, renk `html[data-renk]` ile seçilir. **Varsayılan kip koyu** (`varsayilanTercihler().gorunumKipi = "koyu"`); açık ve "sistemi izle" Ayarlar'dan seçilir. Dört renk teması korunur, koyu tonları daha sakin olacak biçimde hafifçe yumuşatıldı:

| Tema (`data-renk`) | Açık zemin / vurgu | Koyu zemin / vurgu |
| --- | --- | --- |
| `terakota` (varsayılan) | `#f7f4ef` / `#b2411f` | `#151413` / `#e2876a` |
| `okyanus` | `#f2f5f9` / `#1b54c2` | `#0e131a` / `#7aaaf5` |
| `adacayi` | `#f2f6f3` / `#1b6b4c` | `#0f1512` / `#67c79d` |
| `karbon` | `#f4f4f6` / `#3f43c4` | `#111216` / `#9aa0f7` |

- **Cam tokenları:** `--cam`, `--cam-guclu`, `--cam-kenar` (kipe göre); pencere zemininde vurgudan türetilmiş çok hafif köşe ışığı (`--zemin-isik`, radial-gradient).
- **Durum renkleri** (`--durum-rahat/dikkat/kacin/bayat` ve `-bar`) yalnız gerektiğinde: kritik metin, tempo uyarısı (`.dp-tempo.uyari`), Bağla durum noktası.
- **Platform tonları** (`--plt-*`) yalnız isteğe bağlı *tek segmentli halka* dilimlerinde kullanılır; simgeler nötr renktedir.

## 2. Tipografi ve Ölçüler

- **Yazı tipleri (gömülü, OFL):** Inter (arayüz), Source Serif 4 (yalnız `CETVEL` marka adı). Dış CDN yok.
- **En fazla iki boyut:** `--boyut-ana: 13px`, `--boyut-kucuk: 11.5px`. Hiyerarşi boyutla değil ağırlık (600/650) ve renkle (`--metin-soluk`) kurulur.
- **Tabular rakam:** `body { font-feature-settings: "tnum" }` ve `.rakam`.
- **Hareket:** 160/280 ms `cubic-bezier(0.22, 1, 0.36, 1)`; `prefers-reduced-motion: reduce` altında kapalı.

## 3. Görünümler

### 3.1 Açılır pencere (340 × 540)

```
┌ CETVEL v0.1.0 ··········· az önce güncellendi [düzenle] ┐
│ [Genel]  [Claude]  [Antigravity]   ← altta ince çizgi:  │
│ [Codex]  [DeepSeek] [OpenCode]       platformun en darı │
├──────────────────────────────────────────────────────────┤
│  Genel: kompakt kartlar  |  Platform: sade ayrıntı       │
├──────────────────────────────────────────────────────────┤
│ ⟳ Yenile (R)            ⚙ Ayarlar                        │
│ ⓘ Hakkında              ⤢ Genişlet ↵                     │
└──────────────────────────────────────────────────────────┘
```

- **Varsayılan görünüm Genel** (`durum.platformSekme = "genel"`). Sekmeler eşit genişlikli ızgaradır (`repeat(auto-fit, minmax(max(92px, 30%), 1fr))`); dar ekranda alt satıra iner, yatay kaydırma yoktur.
- **Genel kartı** (`kompaktKartHtml`, açılırda `.havuz-satir`): simge + havuz adı, sağda en dar etkin kalan (`%15 kaldı`), altında iki ince çubuk ve sağlarında sıfırlamaya kalan süre. Satırlar `kompaktPencereler`: `5 saat` + `Haftalık`; bunlar yoksa en kısa iki pencere. Antigravity Argon'da 5 saatlik pencere yoktur: boş iz ve `—`. Kotasız sayaç (OpenCode) "kota yok" ve tek satır özet; bakiye (DeepSeek) tutar gösterir.
- **Antigravity'nin üç havuzu Genel'de ayrı kartlardır.** Gerekçe: Düzenle kipindeki sıra/sabitle/gizle tercihleri Rust'a havuz kimliğiyle (`kartSirasi`, `gizliKartlar`, `sabitKartlar`) kaydedilir; tek birleşik kart bu modeli bozar ve kartın içinde ikinci bir liste açardı. Platform sekmesinde ise üç havuz tek ayrıntıda, kısa adla (`Gemini`, `Claude-GPT`, `Argon`) alt alta durur.
- **Kart tıklanınca** (ya da odaktayken Enter) o platformun sekmesi açılır ve ilgili havuza kaydırılır; Düzenle kipinde tıklama yalnız seçer.
- **Platform ayrıntısı** (`platformDetayHtml`): ad + "az önce güncellendi" (ipucu: kaynak · güven · okuma yaşı · notlar); her pencere için başlık (`Oturum (5 saat)`, `Haftalık`), 6 px çubuk (üzerinde soluk doğrusal tempo işareti), altında tek satır `%89 kaldı · 4 sa 12 dk sonra sıfırlanır` ve küçük tempo satırı (`tempoBilgisi`): `Sıfırlamaya kadar yeter`, `Paz 23:22 civarı biter`, `%5 yedekte` / `%5 açıkta`, tükenmiş ya da kilitli pencerede `Sıfırlanınca açılır: …`. Alt sayaçlar (ör. modele özel haftalık sayaç) aynı düzende; ek bilgiler (sıfırlama kredisi, eşzamanlı istek, seviye, kampanya bitişi) tek soluk satırda.
- **30 günlük geçmiş:** ayrıntının altında günlük tepe kullanım çubuk grafiği (`gecmisCubuklariHtml`). Canlıda `gecmis_al { havuz, pencere, gun: 30 }` çağrılır, örnekler yerel güne göre en yüksek `kullanilan` ile toplanır, 5 dk önbelleklenir (`durum.gunlukGecmis`). `?demo=1` kipinde havuz kimliğinden tohumlanan sabit seri kullanılır. Pencere seçimi: `5s`, yoksa belirleyici pencere.
- **Gösterge türü** (Ayarlar → Genel): `cubuk` varsayılan. `halka` ya da `pasta/ayri` seçilirse ayrıntıda her havuzun başında 64 px halka/pasta; `pasta/tek` seçilirse Genel'in üstünde tek segmentli halka.
- **Alt menü:** Yenile (R), Ayarlar, Hakkında (geniş pencerede Ayarlar → Hakkında bölümünü açar), Genişlet. Masaüstünde uygulamadan çıkış tepsi menüsündedir; arayüzün çağırabileceği bir `cikis` komutu yoktur.

### 3.2 Geniş pencere

- Üst çubuk: marka, `Şimdi · Geçmiş · Bağla · Ayarlar`, Düzenle / Yenile / Küçült.
- **Şimdi:** solda platform listesi (`.platform-sekmeler.yan`: ad, `%15 kaldı`, ince çizgi), sağda Genel ızgarası (`.detay-kart` kompakt kartlar, `simdiMasonryGuncelle` ile yerleşim) ya da seçili platformun ayrıntısı (pencereler yan yana, 112 px yüksekliğinde 30 günlük grafik). 680 px altındaki kapsayıcıda liste üstte sekmelere döner.
- **Geçmiş:** 24 saatlik SVG eğri, eşik ve "bu hızla" izdüşümü (işlev aynı; yalnız stil ve pencere etiketleri).
- **Bağla / Ayarlar:** işlev aynı; kartlar camsı, durum rozetleri arka plansız "nokta + metin". Ayarlar'dan *Kart yerleşimi* (birleşik/platform) kaldırıldı; yerini Genel/platform sekmeleri aldı.

### 3.3 Şerit (520 × 44)

Tek satır: tutamak, marka, en dar havuzlar (`CL %15`), `+N`, "en dar" özeti. Öğelerin arka planı yok; kritik olan yalnız kırmızı metin. Sığmayan öğeler `seritSigdir` ile `+N`'ye toplanır.

## 4. Duyarlı Yerleşim ve Erişilebilirlik

- Kapsayıcı sorguları: `.acilir-pencere` ve `.genis-pencere` `container-type: inline-size`; kırılımlar 720 (Ayarlar menüsü), 680 (Şimdi listesi), 560 (Bağla tablosu), 500 (sekme çubuğu) px.
- `araclar/tasma_testi.mjs`: 340 / 480 / 900 / 1400 px × diller × DPR 1 / 1.5 / 2; açılır (Genel, Düzenle, Antigravity ayrıntısı, Claude ayrıntısı + halka), Şimdi (Genel, Düzenle, Claude, Antigravity), Geçmiş, Bağla, Ayarlar'ın 6 bölümü ve şerit ölçülür (yatay taşma, kırpılma, kutudan taşma, binme, SVG metinleri). Ölçüm, dil kodu ve ilk çizim (sekmeler) hazır olunca başlar.
- **Klavye:** `←/→` Genel ve platform sekmeleri arasında (sekmeler `role="tab"`, gezinen `tabindex`), `↑/↓` Genel'de kart seçimi, kart odaktayken `Enter` ayrıntıyı açar (aksi hâlde açılırda Genişlet), `R` yenile, `Esc` önce sürüklemeyi/ipucunu, sonra pencereyi kapatır. Kısayol listesi geniş pencerenin altındaki "Kısayollar" ipucundadır.
- İpucu balonu (`#cetvel-ipucu`) ekran kenarından taşmaz, odakla da açılır; `aria-describedby` bağlanır. Simge düğmelerinin `aria-label`'ı vardır.

## 5. Çok Dilli Altyapı ve Zaman Biçimleme

- Sözlükler `diller/<kod>.json`; kaynak Türkçe gömülü `YEDEK_TR` ile `diller/tr.json` birebir aynı tutulur.
- `tr` ve `en` tam. Diğer 9 dilde önceki çeviriler korunur; bu sürümde eklenen 32 anahtar (`genel.kaldi`, `genel.sonraSifirlanir`, `genel.tempo*`, `genel.gecmis*`, `gorunum.genel`, `pencereEtiket.*`, `pencereBaslik.*` …) `"__CEVIRI__"` + İngilizce taşır ve `meta.tamMi = false` olur; çalışma anında ön ek temizlenir.
- Yüzdeler `yuzdeBicimle` (`Intl.NumberFormat`, `%15` / `15%` / `15 %`), tarih ve saat işletim sisteminin yerel saat dilimiyle biçimlenir. Kalan süre Türkçede `4 g 11 sa`, `1 sa 20 dk`.
- Bütünlük denetimi: `python araclar/dil_denetle.py`.

## 6. Tauri Köprüsü ve Ayarlar

- **Komutlar:** `anlik_al`, `yenile_iste`, `gecmis_al` (24 saat: `gun: 1`, ayrıntı grafiği: `gun: 30`), `servis_durumu`, `servis_baslat`, `tara`, `test_et`, `anahtar_kaydet`, `claude_statusline_onizle`, `claude_statusline_uygula`, `ayarlar_al`, `ayar_kaydet`, `pencere_ac`, `pencere_gizle`, `acilir_gizle`, `serit_gizle`, `serit_durumu_al`, `serit_ustte_ayarla`, `serit_tiklama_gecirgen_ayarla`, `lisans_metni_al`, `gunluk_klasorunu_ac`, `gecmis_disa_aktar`.
- **Olaylar:** `anlik_degisti`, `servis_durumu_degisti`, `pencere_sekme_sec`, `serit_durumu_degisti`, `ayarlar_degisti`. Görsel tercihler (tema, dil, gösterge, değer yönü, ayar bölümü) `localStorage` + `storage` olayıyla pencereler arasında eşitlenir; açılırdaki "Hakkında" bu yolla geniş pencerede Ayarlar → Hakkında'yı açar.
- **Kip varsayılanı:** arayüz varsayılanı koyu; canlı masaüstünde Rust ayarındaki `kip` (Rust varsayılanı `sistem`) geçerlidir.
- Ayarlar 6 bölüm: Genel, Davranış, Bildirimler, Bağlantılar, Teknik, Hakkında ve Yasal.

## 7. Düzenle Kipi (sürükle-bırak, sabitle, gizle)

- Açılırda başlıktaki simge düğmesi, geniş pencerede `Düzenle` ile açılır. Açılınca görünüm Genel'e döner; bir platform sekmesine geçmek düzenlemeyi kapatır.
- Genel kartlarında `⋮⋮` tutamacı (Pointer Events, 6 px eşik, FLIP 200 ms, kenarda otomatik kaydırma; açılırda kaydırma alanı `#acilir-icerik`), yıldızla sabitleme (sabitler en üst grupta), göster/gizle anahtarı ve `Varsayılana dön`. Klavye: tutamakta `Space` al/bırak, oklar taşı, `Esc` iptal; her adım `#ekran-okuyucu-duyuru` ile duyurulur.
- Sıra `localStorage` (`cetvel.ui.kartlar.v1`) ve canlıda `ayar_kaydet({ kartSirasi, gizliKartlar, sabitKartlar })` ile Rust'a yazılır; gizli kartlar sekmelerden, Genel'den ve şeritten kalkar.
