#!/usr/bin/env node
// Cetvel arayüzü taşma ve binme testi.
//
// Ne yapar: apps/arayuz klasörünü yerel bir HTTP sunucusundan (?demo=1) açar, kurulu Chrome ya
// da Edge'i başsız (headless) çalıştırır ve DevTools Protokolü (CDP) ile her birleşimi ölçer:
//
//   genişlik 340 / 480 / 900 / 1400 px × 11 dil × deviceScaleFactor 1 / 1.5 / 2
//   yüzeyler: açılır pencere (Genel, Düzenle, Antigravity ve Claude ayrıntısı), ana pencere (Şimdi
//   Genel/Düzenle/Claude/Antigravity, Geçmiş, Bağla, Ayarlar'ın 6 bölümü), şerit
//
// Ölçülenler:
//   - sayfa yatay taşması (belge genişliği > görünüm),
//   - metin kırpılması (scrollWidth > clientWidth: üç nokta ya da gizli taşma),
//   - kutusundan taşan metin (kart, satır, özet kutusu sınırı dışına çıkan öğe),
//   - birbirine binen metin öğeleri (kesişen dikdörtgenler),
//   - svg metinleri (halka, Geçmiş grafiği eksen ve izdüşüm etiketi): svg dışına taşma ve binme.
//
// Bağımlılık yok: Node 22+ (yerleşik WebSocket) ve kurulu bir Chromium tarayıcısı yeter.
// Playwright gerekmez. Tarayıcı yolu CETVEL_TARAYICI ortam değişkeniyle verilebilir.
//
// Kullanım:
//   node apps/arayuz/araclar/tasma_testi.mjs            # tam tarama (11 dil × 4 genişlik × 3 DPR)
//   node apps/arayuz/araclar/tasma_testi.mjs --hizli    # tr/en/de/ja, DPR 1
//   node apps/arayuz/araclar/tasma_testi.mjs --dil=ru --genislik=340
// Çıkış kodu: sorun yoksa 0, varsa 1, tarayıcı bulunamazsa 2.

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const KOK = resolve(fileURLToPath(new URL("..", import.meta.url)));
const DILLER = ["tr", "en", "de", "fr", "es", "pt-BR", "ru", "ja", "ko", "zh-Hans", "zh-Hant"];
const GENISLIKLER = [340, 480, 900, 1400];
const OLCEKLER = [1, 1.5, 2];
const YUKSEKLIK = 900;
const AYAR_BOLUMLERI = ["genel", "platformlar", "havuzlar", "bildirimler", "teknik", "hakkinda"];

const argv = process.argv.slice(2);
const arg = (ad) => argv.find((a) => a.startsWith(`--${ad}=`))?.split("=")[1];
const hizli = argv.includes("--hizli");
const diller = arg("dil") ? arg("dil").split(",") : hizli ? ["tr", "en", "de", "ja"] : DILLER;
const genislikler = arg("genislik") ? arg("genislik").split(",").map(Number) : GENISLIKLER;
const olcekler = arg("dpr") ? arg("dpr").split(",").map(Number) : hizli ? [1] : OLCEKLER;

// --------------------------------------------------------------------------
// Statik sunucu (yalnız apps/arayuz; yol kaçışı engellenir)
// --------------------------------------------------------------------------
const TURLER = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
};

function sunucuBaslat() {
  const sunucu = createServer((istek, yanit) => {
    const yol = decodeURIComponent(new URL(istek.url, "http://x").pathname);
    const dosya = normalize(join(KOK, yol === "/" ? "index.html" : yol));
    if (!dosya.startsWith(KOK + sep) || !existsSync(dosya) || !statSync(dosya).isFile()) {
      yanit.writeHead(404).end();
      return;
    }
    yanit.writeHead(200, { "content-type": TURLER[extname(dosya)] || "application/octet-stream" });
    yanit.end(readFileSync(dosya));
  });
  return new Promise((ok) => sunucu.listen(0, "127.0.0.1", () => ok(sunucu)));
}

// --------------------------------------------------------------------------
// Tarayıcı + CDP
// --------------------------------------------------------------------------
function tarayiciBul() {
  const adaylar = [
    process.env.CETVEL_TARAYICI,
    join(process.env.ProgramFiles || "C:\\Program Files", "Google\\Chrome\\Application\\chrome.exe"),
    join(process.env["ProgramFiles(x86)"] || "C:\\Program Files (x86)", "Google\\Chrome\\Application\\chrome.exe"),
    join(process.env.LOCALAPPDATA || "", "Google\\Chrome\\Application\\chrome.exe"),
    join(process.env["ProgramFiles(x86)"] || "C:\\Program Files (x86)", "Microsoft\\Edge\\Application\\msedge.exe"),
    join(process.env.ProgramFiles || "C:\\Program Files", "Microsoft\\Edge\\Application\\msedge.exe"),
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  ];
  return adaylar.find((a) => a && existsSync(a));
}

const bekle = (ms) => new Promise((r) => setTimeout(r, ms));

async function tarayiciBaslat(yol) {
  const profil = mkdtempSync(join(tmpdir(), "cetvel-tasma-"));
  const surec = spawn(
    yol,
    [
      "--headless=new",
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-extensions",
      "--hide-scrollbars",
      "--font-render-hinting=none",
      `--user-data-dir=${profil}`,
      "--remote-debugging-port=0",
      "about:blank",
    ],
    { stdio: "ignore" }
  );
  const portDosyasi = join(profil, "DevToolsActivePort");
  for (let i = 0; i < 200 && !existsSync(portDosyasi); i++) await bekle(50);
  if (!existsSync(portDosyasi)) throw new Error("tarayıcı DevTools portunu açmadı");
  let port;
  for (let i = 0; i < 40 && !port; i++) {
    port = readFileSync(portDosyasi, "utf8").split(/\r?\n/)[0];
    if (!port) await bekle(50);
  }
  return {
    port,
    kapat: async () => {
      surec.kill();
      await bekle(300);
      try {
        rmSync(profil, { recursive: true, force: true });
      } catch {
        /* Windows dosyayı hâlâ tutuyorsa geçici klasör kalabilir */
      }
    },
  };
}

class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.sira = 0;
    this.bekleyen = new Map();
    this.olayBekleyen = [];
    this.konsolHatalari = [];
    ws.addEventListener("message", (m) => {
      const v = JSON.parse(m.data);
      if (v.id && this.bekleyen.has(v.id)) {
        const { ok, red } = this.bekleyen.get(v.id);
        this.bekleyen.delete(v.id);
        v.error ? red(new Error(v.error.message)) : ok(v.result);
      } else if (v.method) {
        if (v.method === "Runtime.exceptionThrown") {
          const d = v.params?.exceptionDetails;
          this.konsolHatalari.push(d?.exception?.description || d?.text || "exceptionThrown");
        } else if (v.method === "Runtime.consoleAPICalled" && v.params?.type === "error") {
          const metin = (v.params.args || []).map((a) => a.value ?? a.description ?? "").join(" ");
          this.konsolHatalari.push(metin || "console.error");
        }
        this.olayBekleyen = this.olayBekleyen.filter((b) => !(b.ad === v.method && (b.ok(v.params), true)));
      }
    });
  }
  static async bagla(url) {
    const ws = new WebSocket(url);
    await new Promise((ok, red) => {
      ws.addEventListener("open", ok, { once: true });
      ws.addEventListener("error", red, { once: true });
    });
    return new Cdp(ws);
  }
  gonder(method, params = {}) {
    const id = ++this.sira;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((ok, red) => this.bekleyen.set(id, { ok, red }));
  }
  olay(ad, zamanAsimiMs = 15000) {
    return new Promise((ok, red) => {
      const t = setTimeout(() => red(new Error(`${ad} gelmedi`)), zamanAsimiMs);
      this.olayBekleyen.push({ ad, ok: (p) => (clearTimeout(t), ok(p)) });
    });
  }
  async degerlendir(ifade) {
    const r = await this.gonder("Runtime.evaluate", { expression: ifade, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result.value;
  }
  kapat() {
    this.ws.close();
  }
}

// --------------------------------------------------------------------------
// Sayfa içi ölçüm (tarayıcıda çalışır)
// --------------------------------------------------------------------------
const OLCUM = String.raw`(() => {
  const sorunlar = [];
  const gorunur = (el) => {
    if (!el.isConnected) return false;
    const s = getComputedStyle(el);
    if (s.display === "none" || s.visibility === "hidden" || Number(s.opacity) === 0) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0.5 && r.height > 0.5;
  };
  const kimlik = (el) => {
    let ad = el.tagName.toLowerCase();
    if (el.id) ad += "#" + el.id;
    const sinif = [...el.classList].slice(0, 2).join(".");
    if (sinif) ad += "." + sinif;
    const metin = (el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 40);
    return ad + (metin ? ' "' + metin + '"' : "");
  };
  const yuzey = document.querySelector(".yuzey-kapsayici.aktif") || document.body;
  // Kaydırılabilir olduğu bilinen alanlar (tablo sarıcı, lisans metni, girdi) kırpılma sayılmaz.
  const KAYDIRMALI = ".veri-kaynagi-tablo-sarici, .lisans-metin, input, select, textarea, option, svg, .gecmis-grafik";
  const KUTU = ".havuz-satir, .detay-kart, .ozet-kutu, .bagla-kart, .ayar-satir, .serit, .acilir-pencere, .genis-pencere";

  const belge = document.documentElement;
  if (belge.scrollWidth > innerWidth + 1) {
    sorunlar.push({ tur: "sayfa_yatay_tasma", oge: "html", ayrinti: belge.scrollWidth + " > " + innerWidth });
  }

  const tumu = [...yuzey.querySelectorAll("*")].filter((el) => !el.closest(KAYDIRMALI) && !el.closest(".gizli, [hidden]"));
  const metinli = [];
  for (const el of tumu) {
    const dogrudanMetin = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
    if (!dogrudanMetin || !gorunur(el)) continue;
    metinli.push(el);
    const s = getComputedStyle(el);
    // 1) Kırpılma: içerik kutusundan geniş (üç nokta ya da gizli taşma).
    // Sağlayıcıdan gelen dinamik etiketler (.kisalt-etiket + data-ipucu) bilinçli ellipsis ile kısaltılır.
    const kasitliKisalt =
      el.classList.contains("kisalt-etiket") &&
      s.textOverflow === "ellipsis" &&
      Boolean(el.dataset?.ipucu || el.closest("[data-ipucu]"));
    if (!kasitliKisalt && el.scrollWidth > el.clientWidth + 1 && s.overflowX !== "visible" && s.display !== "inline") {
      sorunlar.push({ tur: "kirpilma", oge: kimlik(el), ayrinti: el.scrollWidth + " > " + el.clientWidth });
    }
    if (el.scrollHeight > el.clientHeight + 2 && s.overflowY === "hidden" && s.display !== "inline") {
      sorunlar.push({ tur: "dikey_kirpilma", oge: kimlik(el), ayrinti: el.scrollHeight + " > " + el.clientHeight });
    }
    // 2) Kutusundan taşma.
    const kutu = el.parentElement?.closest(KUTU);
    if (kutu && gorunur(kutu)) {
      const a = el.getBoundingClientRect();
      const b = kutu.getBoundingClientRect();
      if (a.left < b.left - 1 || a.right > b.right + 1) {
        sorunlar.push({ tur: "kutudan_tasma", oge: kimlik(el), ayrinti: "kutu " + kimlik(kutu).slice(0, 40) });
      }
    }
    // 3) Görünüm dışına taşma.
    const r = el.getBoundingClientRect();
    if (r.right > innerWidth + 1 || r.left < -1) {
      sorunlar.push({ tur: "gorunum_disi", oge: kimlik(el), ayrinti: Math.round(r.left) + ".." + Math.round(r.right) });
    }
  }

  // 4) Binme: birbirini içermeyen metin öğelerinin metin kutuları kesişmesin. Kaydırma alanında
  // görünmeyen kısım (ör. liste altı, sabit alt çubuğun altında kalan satır) kırpılarak ölçülür.
  const kirpmaKutusu = (el) => {
    let k = { left: -Infinity, top: -Infinity, right: Infinity, bottom: Infinity };
    for (let a = el.parentElement; a; a = a.parentElement) {
      const s = getComputedStyle(a);
      if (s.overflowX !== "visible" || s.overflowY !== "visible") {
        const r = a.getBoundingClientRect();
        k = { left: Math.max(k.left, r.left), top: Math.max(k.top, r.top), right: Math.min(k.right, r.right), bottom: Math.min(k.bottom, r.bottom) };
      }
    }
    return k;
  };
  const metinKutusu = (el) => {
    const rng = document.createRange();
    const kirp = kirpmaKutusu(el);
    const kutular = [];
    for (const n of el.childNodes) {
      if (n.nodeType !== 3 || !n.textContent.trim()) continue;
      rng.selectNodeContents(n);
      for (const k of rng.getClientRects()) {
        const r = { left: Math.max(k.left, kirp.left), top: Math.max(k.top, kirp.top), right: Math.min(k.right, kirp.right), bottom: Math.min(k.bottom, kirp.bottom) };
        if (r.right - r.left > 0.5 && r.bottom - r.top > 0.5) kutular.push(r);
      }
    }
    return kutular;
  };
  const kayitlar = metinli.map((el) => ({ el, kutular: metinKutusu(el) })).filter((k) => k.kutular.length);
  for (let i = 0; i < kayitlar.length; i++) {
    for (let j = i + 1; j < kayitlar.length; j++) {
      const A = kayitlar[i];
      const B = kayitlar[j];
      if (A.el.contains(B.el) || B.el.contains(A.el)) continue;
      let binme = false;
      for (const a of A.kutular) {
        for (const b of B.kutular) {
          const gx = Math.min(a.right, b.right) - Math.max(a.left, b.left);
          const gy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
          if (gx > 1.5 && gy > 1.5) binme = true;
        }
      }
      if (binme) sorunlar.push({ tur: "binme", oge: kimlik(A.el), ayrinti: "↔ " + kimlik(B.el) });
    }
  }
  // 5) SVG metinleri (halka merkezi, Geçmiş grafiği eksen ve izdüşüm etiketleri): yukarıdaki HTML
  // ölçümü svg'yi dışarıda bırakır; burada her svg'nin metinleri svg sınırı içinde kalmalı ve
  // birbirine binmemeli. Yerel saat biçimi uzun dillerde (ru, ja, zh, ko) en dar ekranda da ölçülür.
  let svgOlculen = 0;
  for (const svg of yuzey.querySelectorAll("svg")) {
    if (!gorunur(svg) || svg.closest(".gizli, [hidden]")) continue;
    const sr = svg.getBoundingClientRect();
    const kirp = kirpmaKutusu(svg);
    if (sr.bottom < kirp.top || sr.top > kirp.bottom || sr.right < kirp.left || sr.left > kirp.right) continue;
    const yazilar = [...svg.querySelectorAll("text")]
      .filter((m) => (m.textContent || "").trim() && gorunur(m))
      .map((m) => ({ m, r: m.getBoundingClientRect() }))
      .filter((k) => k.r.width > 0.5 && k.r.height > 0.5);
    svgOlculen += yazilar.length;
    for (const k of yazilar) {
      if (k.r.left < sr.left - 1 || k.r.right > sr.right + 1 || k.r.top < sr.top - 1 || k.r.bottom > sr.bottom + 1) {
        sorunlar.push({ tur: "svg_tasma", oge: kimlik(k.m), ayrinti: "svg " + Math.round(sr.left) + ".." + Math.round(sr.right) });
      }
      if (k.r.right > innerWidth + 1 || k.r.left < -1) {
        sorunlar.push({ tur: "gorunum_disi", oge: kimlik(k.m), ayrinti: Math.round(k.r.left) + ".." + Math.round(k.r.right) });
      }
    }
    for (let i = 0; i < yazilar.length; i++) {
      for (let j = i + 1; j < yazilar.length; j++) {
        const a = yazilar[i].r;
        const b = yazilar[j].r;
        const gx = Math.min(a.right, b.right) - Math.max(a.left, b.left);
        const gy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
        if (gx > 1.5 && gy > 1.5) {
          sorunlar.push({ tur: "svg_binme", oge: kimlik(yazilar[i].m), ayrinti: "↔ " + kimlik(yazilar[j].m) });
        }
      }
    }
  }
  return { sorunlar, olculen: metinli.length, svgOlculen };
})()`;

// --------------------------------------------------------------------------
// Senaryolar
// --------------------------------------------------------------------------
function senaryolar() {
  const liste = [
    { ad: "acilir", yuzey: "acilir", hazirla: "" },
    {
      ad: "acilir-duzenle",
      yuzey: "acilir",
      hazirla: `document.querySelector('#btn-acilir-duzenle')?.click();`,
      dogrula: `!!document.querySelector('#acilir-havuz-listesi .kart-tutamak') && !!document.querySelector('#acilir-duzenle-cubuk:not(.gizli)')`,
      dogrulamaAdi: "acilir_duzenle_acilmadi",
    },
    // Platform ayrıntısı: en kalabalık sekme Antigravity (4 aktif + 1 görülmeyen havuz), Codex (48 karakterlik etiket) ve Claude (alt sayaç ağacı + 30 günlük geçmiş).
    {
      ad: "acilir-agy",
      yuzey: "acilir",
      hazirla: `document.querySelector('#acilir-platform-sekmeler [data-platform-sekme="agy"]')?.click();`,
      dogrula: `document.querySelectorAll('#acilir-detay .detay-havuz').length >= 3 && !!document.querySelector('#acilir-detay .alt-sekme-cubuk') && !!document.querySelector('#acilir-detay .gorulmeyen-bolum')`,
      dogrulamaAdi: "agy_ayrintisi_cizilmedi",
    },
    {
      ad: "acilir-agy-alt",
      yuzey: "acilir",
      hazirla: `document.querySelector('#acilir-platform-sekmeler [data-platform-sekme="agy"]')?.click(); document.querySelector('#acilir-detay [data-alt-sekme="agy_claude_gpt"]')?.click(); document.querySelector('#acilir-detay [data-gorulmeyen-toggle="agy"]')?.click();`,
      dogrula: `document.querySelectorAll('#acilir-detay .detay-havuz').length === 1 && !!document.querySelector('#acilir-detay .gorulmeyen-liste')`,
      dogrulamaAdi: "agy_alt_sekme_cizilmedi",
    },
    {
      ad: "acilir-codex",
      yuzey: "acilir",
      hazirla: `document.querySelector('#acilir-platform-sekmeler [data-platform-sekme="codex"]')?.click();`,
      dogrula: `document.querySelectorAll('#acilir-detay .detay-havuz').length >= 2 && !!document.querySelector('#acilir-detay .alt-sekme-cubuk')`,
      dogrulamaAdi: "codex_ayrintisi_cizilmedi",
    },
    {
      ad: "acilir-claude-halka",
      yuzey: "acilir",
      hazirla: `document.querySelector('[data-gosterge-sec="halka"]')?.click(); document.querySelector('#acilir-platform-sekmeler [data-platform-sekme="claude"]')?.click();`,
      dogrula: `!!document.querySelector('#acilir-detay .gm-cubuklar') && !!document.querySelector('#acilir-detay .alt-sayac-agac')`,
      dogrulamaAdi: "claude_ayrintisi_cizilmedi",
    },
    {
      ad: "acilir-baglam",
      yuzey: "acilir",
      hazirla: `const k = document.querySelector('#acilir-havuz-listesi .kompakt-kart'); if (k) { const r = k.getBoundingClientRect(); k.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.left + 40, clientY: r.top + 20 })); }`,
      dogrula: `!!document.querySelector('#cetvel-baglam-menu:not(.gizli) [role="menuitem"]')`,
      dogrulamaAdi: "baglam_menusu_acilmadi",
    },
    { ad: "serit", yuzey: "serit", hazirla: "" },
    {
      ad: "simdi-claude",
      yuzey: "pencere",
      hazirla: `document.querySelector('#tab-simdi').click(); document.querySelector('#simdi-platform-sekmeler [data-platform-sekme="claude"]')?.click();`,
      dogrula: `!!document.querySelector('#simdi-detay .gm-cubuklar') && !!document.querySelector('#simdi-detay .alt-sayac-agac') && !!document.querySelector('#simdi-ekmek:not(.gizli)')`,
      dogrulamaAdi: "genis_ayrinti_cizilmedi",
    },
    {
      ad: "simdi-agy",
      yuzey: "pencere",
      hazirla: `document.querySelector('#tab-simdi').click(); document.querySelector('#simdi-platform-sekmeler [data-platform-sekme="agy"]')?.click();`,
    },
    {
      ad: "simdi-agy-ekmek",
      yuzey: "pencere",
      hazirla: `document.querySelector('#tab-simdi').click(); document.querySelector('#simdi-platform-sekmeler [data-platform-sekme="agy"]')?.click(); document.querySelector('#simdi-detay [data-alt-sekme="agy_claude_gpt"]')?.click();`,
      dogrula: `!!document.querySelector('#simdi-ekmek.uc-halka:not(.gizli)')`,
      dogrulamaAdi: "ekmek_kirintisi_cizilmedi",
    },
    {
      ad: "simdi-codex-uzun",
      yuzey: "pencere",
      hazirla: `document.querySelector('#tab-simdi').click(); document.querySelector('#simdi-platform-sekmeler [data-platform-sekme="codex"]')?.click(); document.querySelector('#simdi-detay [data-alt-sekme="codex_spark"]')?.click();`,
      dogrula: `document.querySelectorAll('#simdi-detay .detay-havuz').length === 1 && !!document.querySelector('#simdi-ekmek.uc-halka:not(.gizli)')`,
      dogrulamaAdi: "codex_uzun_etiket_cizilmedi",
    },
    { ad: "simdi", yuzey: "pencere", hazirla: `document.querySelector('#tab-simdi').click();` },
    {
      ad: "simdi-duzenle",
      yuzey: "pencere",
      hazirla: `document.querySelector('#tab-simdi').click(); document.querySelector('#btn-genis-duzenle')?.click();`,
      dogrula: `!!document.querySelector('#simdi-havuz-grid .kart-tutamak') && !!document.querySelector('#simdi-duzenle-cubuk:not(.gizli)')`,
      dogrulamaAdi: "simdi_duzenle_acilmadi",
    },
    { ad: "bagla", yuzey: "pencere", hazirla: `document.querySelector('#tab-bagla').click();` },
    {
      ad: "gecmis",
      yuzey: "pencere",
      hazirla: `document.querySelector('#tab-gecmis').click();`,
      // İzdüşüm dilimi (kesikli çizgi + sağ uçta sıfırlama saati) çizilmeli; yoksa ölçüm onu kapsamaz.
      dogrula: `!!document.querySelector('.gecmis-svg path[stroke-dasharray="5 4"][fill="none"]') && document.querySelectorAll('.gecmis-svg text').length >= 7`,
      dogrulamaAdi: "izdusum_cizilmedi",
    },
    {
      ad: "ayarlar-arama",
      yuzey: "pencere",
      hazirla: `document.querySelector('#tab-ayarlar').click(); const g = document.querySelector('#ayar-arama-girdi'); if (g) { g.value = 'Claude'; g.dispatchEvent(new Event('input', { bubbles: true })); }`,
      dogrula: `document.querySelectorAll('#ayarlar-grid .ayar-satir, #ayarlar-grid .havuz-ayar-satir').length >= 1`,
      dogrulamaAdi: "ayar_aramasi_calismadi",
    },
  ];
  for (const b of AYAR_BOLUMLERI) {
    liste.push({
      ad: `ayarlar/${b}`,
      yuzey: "pencere",
      hazirla: `document.querySelector('#tab-ayarlar').click(); document.querySelector('[data-ayar-bolum="${b}"]')?.click();`,
      ...(b === "havuzlar"
        ? {
            dogrula: `document.querySelectorAll('#ayarlar-grid .expander-kart').length >= 2 && document.querySelectorAll('#ayarlar-grid .havuz-ayar-satir').length >= 1`,
            dogrulamaAdi: "havuzlar_ayari_cizilmedi",
          }
        : {}),
    });
  }
  return liste;
}

async function main() {
  const tarayici = tarayiciBul();
  if (!tarayici) {
    console.error("Chrome ya da Edge bulunamadı; CETVEL_TARAYICI ile yol verin.");
    process.exit(2);
  }
  const sunucu = await sunucuBaslat();
  const adres = `http://127.0.0.1:${sunucu.address().port}`;
  const t = await tarayiciBaslat(tarayici);
  let cdp;
  const sonuclar = [];
  let birlesim = 0;
  let svgMetinSayisi = 0;
  try {
    const hedef = await (await fetch(`http://127.0.0.1:${t.port}/json/new?about:blank`, { method: "PUT" })).json();
    cdp = await Cdp.bagla(hedef.webSocketDebuggerUrl);
    await cdp.gonder("Page.enable");
    await cdp.gonder("Runtime.enable");

    for (const dil of diller) {
      for (const sen of senaryolar()) {
        // Dil ve yüzey tercihleri sayfa açılmadan yerleşir; demo veri sabittir.
        const tercih = JSON.stringify({ uiDil: dil, ayarBolum: "genel" });
        const ekle = await cdp.gonder("Page.addScriptToEvaluateOnNewDocument", {
          source: `try { localStorage.setItem("cetvel.ui.tercihler.v2", ${JSON.stringify(tercih)}); } catch (e) {}`,
        });
        for (const genislik of genislikler) {
          for (const olcek of olcekler) {
            await cdp.gonder("Emulation.setDeviceMetricsOverride", {
              width: genislik,
              height: YUKSEKLIK,
              deviceScaleFactor: olcek,
              mobile: false,
            });
            cdp.konsolHatalari = [];
            const yuklendi = cdp.olay("Page.loadEventFired");
            await cdp.gonder("Page.navigate", { url: `${adres}/index.html?demo=1&yuzey=${sen.yuzey}` });
            await yuklendi;
            // Dil dosyası yüklensin, düzen otursun.
            await cdp.degerlendir(`new Promise((ok) => {
              const kod = ${JSON.stringify(dil)};
              const t0 = Date.now();
              (function bak() {
                // Dil kodu çizimden önce yazılır: ilk çizimin (sekmeler) bitmesini de bekle.
                const cizildi = !!document.querySelector("#acilir-platform-sekmeler [data-platform-sekme]");
                if ((document.documentElement.lang === kod && cizildi) || Date.now() - t0 > 3000) ok(true);
                else setTimeout(bak, 30);
              })();
            })`);
            if (sen.hazirla) await cdp.degerlendir(sen.hazirla);
            await cdp.degerlendir(`document.fonts.ready.then(() => new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(ok, 40)))))`);
            const dilYuklu = await cdp.degerlendir(`document.documentElement.lang`);
            const olcum = await cdp.degerlendir(OLCUM);
            birlesim++;
            if (dilYuklu !== dil) olcum.sorunlar.push({ tur: "dil_yuklenmedi", oge: "html", ayrinti: String(dilYuklu) });
            for (const kh of cdp.konsolHatalari) {
              olcum.sorunlar.push({ tur: "konsol_hatasi", oge: sen.ad, ayrinti: String(kh).slice(0, 120) });
            }
            if (sen.dogrula && !(await cdp.degerlendir(sen.dogrula))) {
              olcum.sorunlar.push({ tur: sen.dogrulamaAdi, oge: sen.ad, ayrinti: "senaryo beklenen öğeyi çizmedi" });
            }
            svgMetinSayisi += olcum.svgOlculen || 0;
            for (const s of olcum.sorunlar) sonuclar.push({ dil, genislik, olcek, senaryo: sen.ad, ...s });
          }
        }
        await cdp.gonder("Page.removeScriptToEvaluateOnNewDocument", { identifier: ekle.identifier });
      }
      process.stdout.write(`. ${dil}\n`);
    }
  } finally {
    cdp?.kapat();
    await t.kapat();
    sunucu.close();
  }

  // Aynı sorun birçok ölçekte tekrar eder: öğe + tür başına tek satır, birleşim sayısıyla.
  const gruplar = new Map();
  for (const s of sonuclar) {
    const anahtar = `${s.senaryo} | ${s.tur} | ${s.oge}`;
    const g = gruplar.get(anahtar) || { ...s, adet: 0, yerler: new Set() };
    g.adet++;
    g.yerler.add(`${s.dil}@${s.genislik}x${s.olcek}`);
    gruplar.set(anahtar, g);
  }
  console.log(`\n${birlesim} birleşim ölçüldü (${diller.length} dil × ${genislikler.length} genişlik × ${olcekler.length} ölçek × ${senaryolar().length} ekran; ${svgMetinSayisi} svg metni dahil).`);
  if (gruplar.size === 0) {
    console.log("Taşma, kırpılma ya da binme yok.");
    return 0;
  }
  console.log(`${sonuclar.length} sorun, ${gruplar.size} benzersiz:`);
  const sirali = [...gruplar.values()].sort((a, b) => b.adet - a.adet);
  for (const g of sirali.slice(0, 80)) {
    const yer = [...g.yerler].slice(0, 4).join(", ") + (g.yerler.size > 4 ? ` +${g.yerler.size - 4}` : "");
    console.log(`- [${g.senaryo}] ${g.tur}: ${g.oge} (${g.ayrinti}) × ${g.adet} · ${yer}`);
  }
  return 1;
}

main().then(
  (kod) => process.exit(kod),
  (e) => {
    console.error(e);
    process.exit(1);
  }
);
