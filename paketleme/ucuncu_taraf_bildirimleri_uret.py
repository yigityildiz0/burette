#!/usr/bin/env python3
"""THIRD-PARTY-NOTICES.txt üretir: dağıtılan ikililere (cetvel.exe, cetvel-masaustu.exe) giren
Rust bağımlılıklarının adı, sürümü, lisans ifadesi, kaynak adresi ve paketle gelen lisans metinleri.

Kaynak: `cargo metadata` (yalnız Windows x64 hedefi, yalnız normal bağımlılıklar; derleme ve test
bağımlılıkları ikiliye girmez). Lisans metinleri her paketin kendi klasöründeki LICENSE*/COPYING*/
NOTICE* dosyalarından alınır; aynı metin bir kez yazılır ve paketler ona bağlanır.

Çıktıda yerel yol bulunmaz. Kullanım (depo kökünden):

    python paketleme/ucuncu_taraf_bildirimleri_uret.py            # yazar
    python paketleme/ucuncu_taraf_bildirimleri_uret.py --denetle  # güncel mi (CI), değilse çıkış 1
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

DEPO = Path(__file__).resolve().parent.parent
CIKTI = DEPO / "THIRD-PARTY-NOTICES.txt"
HEDEF = "x86_64-pc-windows-msvc"
DAGITILAN = ["cetvel", "cetvel-masaustu"]
LISANS_DOSYASI = re.compile(r"^(LICEN[SC]E|COPYING|NOTICE|UNLICENSE)([-._].*)?$", re.IGNORECASE)
# Lisans metni sayılmayacak uzantılar (kaynak kod, betik).
ATLA_UZANTI = {".rs", ".py", ".sh", ".toml", ".json", ".yml", ".yaml", ".html", ".js"}

# Paket klasöründe lisans dosyası taşımayan ve aynı depoda dosyalı kardeşi de olmayan bileşenler
# için, depodaki LICENSE dosyasından elle doğrulanmış telif satırı (depo adresi → satır). Yeni bir
# dosyasız bileşen eklenirse betik durur; satır kaynağından doğrulanıp buraya eklenmelidir.
DOGRULANMIS_TELIF = {
    # https://github.com/wravery/webview2-rs/blob/main/LICENSE (MIT), 2026-11-16 doğrulandı.
    "https://github.com/wravery/webview2-rs": "Copyright (c) 2021 Bill Avery",
}

# Telif satırı şablona eklenerek yeniden üretilen standart SPDX metinleri.
SPDX_SABLON = {
    "MIT": """MIT License

{telif}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.""",
}

# Telif satırı gerektirmeyen lisanslar: aynı lisansı taşıyan başka bir bileşenin metni (imzasıyla
# tanınır) olduğu gibi kullanılır.
TELIFSIZ_IMZA = {
    "MPL-2.0": "Mozilla Public License Version 2.0",
}


def depo_anahtari(paket: dict) -> str:
    adres = (paket.get("repository") or "").strip().rstrip("/")
    return adres.removesuffix(".git").lower()


def metadata() -> dict:
    cikti = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", HEDEF],
        cwd=DEPO,
        check=True,
        capture_output=True,
    ).stdout
    return json.loads(cikti)


def dagitilan_paketler(meta: dict) -> list[dict]:
    paketler = {p["id"]: p for p in meta["packages"]}
    dugumler = {d["id"]: d for d in meta["resolve"]["nodes"]}
    uyeler = set(meta["workspace_members"])
    kokler = [p["id"] for p in meta["packages"] if p["id"] in uyeler and p["name"] in DAGITILAN]
    gorulen: set[str] = set()
    yigin = list(kokler)
    while yigin:
        kimlik = yigin.pop()
        if kimlik in gorulen:
            continue
        gorulen.add(kimlik)
        for bag in dugumler[kimlik]["deps"]:
            turler = {k.get("kind") for k in bag.get("dep_kinds", [])}
            # None = normal bağımlılık; "build"/"dev" ikiliye girmez.
            if None in turler:
                yigin.append(bag["pkg"])
    secilen = [paketler[k] for k in gorulen if k not in uyeler]
    return sorted(secilen, key=lambda p: (p["name"].lower(), p["version"]))


def lisans_metinleri(paket: dict) -> list[tuple[str, str]]:
    klasor = Path(paket["manifest_path"]).parent
    sonuc = []
    for dosya in sorted(klasor.iterdir(), key=lambda d: d.name.lower()):
        if not dosya.is_file() or not LISANS_DOSYASI.match(dosya.name):
            continue
        if dosya.suffix.lower() in ATLA_UZANTI or dosya.stat().st_size > 256 * 1024:
            continue
        try:
            metin = dosya.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        metin = metin.replace("\r\n", "\n").strip("\n")
        if metin:
            sonuc.append((dosya.name, metin))
    return sonuc


def uret(meta: dict) -> str:
    paketler = dagitilan_paketler(meta)
    metinler: dict[str, str] = {}
    metin_paketleri: dict[str, list[str]] = {}
    satirlar = []

    def metin_ekle(metin: str, etiket: str) -> str:
        ozet = hashlib.sha256(metin.encode("utf-8")).hexdigest()[:12]
        metinler.setdefault(ozet, metin)
        metin_paketleri.setdefault(ozet, []).append(etiket)
        return ozet

    # 1. geçiş: paketin kendi klasöründeki lisans dosyaları.
    dosyalar = {id(p): lisans_metinleri(p) for p in paketler}
    # Depo → (paket, lisans ifadesi, dosya adı, metin): kardeş yalnız aynı lisans ifadesindeyse
    # kullanılır (ör. servo/stylo deposunda MIT/Apache ve MPL paketleri bir arada).
    depo_metinleri: dict[str, list[tuple[str, str, str, str]]] = {}
    for p in paketler:
        for dosya_adi, metin in dosyalar[id(p)]:
            depo_metinleri.setdefault(depo_anahtari(p), []).append(
                (f"{p['name']} {p['version']}", p.get("license") or "", dosya_adi, metin)
            )

    eksikler: list[str] = []
    for p in paketler:
        ad = f"{p['name']} {p['version']}"
        lisans = p.get("license") or ("(özel lisans dosyası)" if p.get("license_file") else "(belirtilmemiş)")
        kaynak = p.get("repository") or p.get("homepage") or f"https://crates.io/crates/{p['name']}"
        anahtarlar = [metin_ekle(metin, f"{ad} ({dosya_adi})") for dosya_adi, metin in dosyalar[id(p)]]
        aciklama = ""
        if not anahtarlar:
            # 2. geçiş: paket dosya taşımıyorsa metin yine de eksiksiz yeniden üretilir.
            depo = depo_anahtari(p)
            kardes = (
                [k for k in depo_metinleri.get(depo, []) if k[0] != ad and k[1] == lisans]
                if depo
                else []
            )
            telifsiz = TELIFSIZ_IMZA.get(lisans)
            telif = DOGRULANMIS_TELIF.get(depo)
            if kardes:
                # Aynı depodaki kardeş paketin LICENSE dosyası (aynı telif ve izin bildirimi).
                anahtarlar = [metin_ekle(m, f"{ad} (aynı depo: {k} {d})") for k, _, d, m in kardes]
                aciklama = " (aynı depodaki lisans dosyası)"
            elif telifsiz and (
                hazir := next((m for ds in dosyalar.values() for _, m in ds if telifsiz in m), None)
            ):
                anahtarlar = [metin_ekle(hazir, f"{ad} (standart {lisans} metni)")]
                aciklama = f" (standart {lisans} metni)"
            elif lisans in SPDX_SABLON and telif:
                metin = SPDX_SABLON[lisans].format(telif=telif)
                anahtarlar = [metin_ekle(metin, f"{ad} (depo LICENSE telifiyle {lisans} metni)")]
                aciklama = f" ({lisans} metni, telif: depo LICENSE)"
            else:
                eksikler.append(f"{ad} [{lisans}] {kaynak}")
        ek = f"  metin: {', '.join('#' + a for a in anahtarlar)}{aciklama}"
        satirlar.append(f"- {ad}\n  lisans: {lisans}\n  kaynak: {kaynak}\n{ek}")

    if eksikler:
        raise SystemExit(
            "Lisans metni bulunamayan bileşen(ler); telif satırını kaynağından doğrulayıp "
            "DOGRULANMIS_TELIF'e ekleyin:\n  " + "\n  ".join(eksikler)
        )

    lisans_ozeti: dict[str, int] = {}
    for p in paketler:
        anahtar = p.get("license") or "(diğer)"
        lisans_ozeti[anahtar] = lisans_ozeti.get(anahtar, 0) + 1

    parcalar = [
        "ÜÇÜNCÜ TARAF BİLDİRİMLERİ / THIRD-PARTY NOTICES",
        "=" * 48,
        "",
        "Cetvel, MIT ya da Apache-2.0 lisansıyla (seçim sizin) dağıtılır; bkz. LICENSE-MIT ve",
        "LICENSE-APACHE. Cetvel ikilileri aşağıdaki açık kaynak bileşenleri içerir; her biri kendi",
        "lisansına tabidir. Bu dosya `paketleme/ucuncu_taraf_bildirimleri_uret.py` ile Cargo.lock'tan",
        "üretilir; elle düzenlenmez.",
        "",
        "Cetvel is dual-licensed under MIT or Apache-2.0 (see LICENSE-MIT and LICENSE-APACHE). The",
        "Cetvel binaries include the open-source components listed below, each under its own license.",
        "",
        f"Hedef / target: {HEDEF} · bileşen sayısı / components: {len(paketler)}",
        "",
        "Lisans dağılımı / License summary:",
        *[f"  {say:4d}  {lisans}" for lisans, say in sorted(lisans_ozeti.items(), key=lambda x: (-x[1], x[0]))],
        "",
        "Arayüz (apps/arayuz) dış kitaplık, yazı tipi ya da görsel içermez; tümü Cetvel'e aittir.",
        "Platform adları (Claude, Codex, Antigravity, DeepSeek, OpenCode, DeepSeek Harness)",
        "yalnız tanımlayıcı olarak kullanılır; sahiplerinin markalarıdır, Cetvel onlarla bağlı değildir.",
        "",
        "BİLEŞENLER / COMPONENTS",
        "-" * 23,
        "",
        "\n".join(satirlar),
        "",
        "LİSANS METİNLERİ / LICENSE TEXTS",
        "-" * 32,
    ]
    for ozet, metin in metinler.items():
        kullananlar = metin_paketleri[ozet]
        parcalar.append("")
        parcalar.append(f"#{ozet} — {len(kullananlar)} bileşen: " + "; ".join(kullananlar))
        parcalar.append("~" * 72)
        parcalar.append(metin)
    cikti = "\n".join(parcalar).rstrip() + "\n"
    # Güvenlik: yerel yol sızmasın.
    for yasak in (str(Path.home()), str(DEPO)):
        if yasak and yasak.lower() in cikti.lower():
            raise SystemExit(f"çıktıda yerel yol var: {yasak!r}")
    return cikti


def main() -> int:
    ayr = argparse.ArgumentParser(description=__doc__)
    ayr.add_argument("--denetle", action="store_true", help="dosya güncel değilse çıkış 1")
    a = ayr.parse_args()
    metin = uret(metadata())
    if a.denetle:
        # Git autocrlf çalışma kopyasını CRLF yapabilir; satır sonu farkı güncellik sayılmaz.
        mevcut = CIKTI.read_bytes().decode("utf-8").replace("\r\n", "\n") if CIKTI.exists() else ""
        if mevcut != metin:
            print("THIRD-PARTY-NOTICES.txt güncel değil; betiği çalıştırın.", file=sys.stderr)
            return 1
        print("THIRD-PARTY-NOTICES.txt güncel.")
        return 0
    CIKTI.write_text(metin, encoding="utf-8", newline="\n")
    print(f"Yazıldı: {CIKTI.name} ({len(metin.encode('utf-8')) // 1024} KiB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
